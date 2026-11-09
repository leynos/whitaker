//! The stub `cargo` the forwarding probe puts first on `PATH`.
//!
//! The root CLI forwards a lint run through the bootstrap an install records,
//! so the probe must not start the real Dylint. The stub answers the
//! bootstrap's `dylint --version` check, records the library path the
//! forwarded invocation would have exported, and exits with
//! [`STUB_EXIT_CODE`] so the test can prove the exit travelled through.
//!
//! The stub lives in its own module because the fixture module also owns the
//! install command, the roots, and the toolchain probes; keeping the stand-in
//! `cargo` here keeps each file within the repository's module size limit.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// The stub `cargo`'s log file, relative to the directory holding it.
const STUB_LOG_NAME: &str = "stub.log";

/// The exit code the stub reports, so the test can prove it survived.
pub const STUB_EXIT_CODE: i32 = 23;

/// The stub `cargo` script itself.
const STUB_CARGO: &str = r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == dylint && "${2:-}" == --version ]]; then
    echo "cargo-dylint 6.3.0"
    exit 0
fi
printf 'LIB=%s\n' "${DYLINT_LIBRARY_PATH:-}" >>"${STUB_LOG}"
exit "${STUB_EXIT:-0}"
"#;

/// A stub `cargo`, its own directory, and the log it writes.
pub struct StubCargo {
    directory: TempDir,
    log: PathBuf,
}

impl StubCargo {
    /// Write the stub into a directory of its own.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed directory or file creation.
    pub fn new() -> Result<Self, String> {
        let directory = super::temporary_directory("stub cargo")?;
        let log = directory.path().join(STUB_LOG_NAME);
        write_executable(&directory.path().join("cargo"), STUB_CARGO)?;
        Ok(Self { directory, log })
    }

    /// The `DYLINT_LIBRARY_PATH` the forwarded invocation was given.
    ///
    /// # Errors
    ///
    /// Returns a description of a log the stub never wrote or one that records
    /// no library path.
    pub fn library_path(&self) -> Result<String, String> {
        let log = std::fs::read_to_string(&self.log).map_err(|error| {
            format!(
                "{} should record the forwarded run: {error}",
                self.log.display()
            )
        })?;
        log.lines()
            .rev()
            .find_map(|line| line.strip_prefix("LIB=").map(ToOwned::to_owned))
            .ok_or_else(|| {
                format!("the forwarded run should record DYLINT_LIBRARY_PATH, got:\n{log}")
            })
    }

    /// The log file the stub appends to.
    pub fn log_path(&self) -> &Path { &self.log }

    /// The inherited `PATH` with the stub's directory first.
    ///
    /// # Errors
    ///
    /// Returns a description of a `PATH` that cannot be joined.
    pub(super) fn path_with_stub(&self) -> Result<std::ffi::OsString, String> {
        let mut entries = vec![self.directory.path().to_path_buf()];
        entries.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(entries)
            .map_err(|error| format!("the fixture PATH should be valid: {error}"))
    }
}

/// Write `script` as an executable file.
///
/// # Errors
///
/// Returns a description of the failed write or permission change.
fn write_executable(path: &Path, script: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(path, script)
        .map_err(|error| format!("{} should be written: {error}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("{} should be executable: {error}", path.display()))
}
