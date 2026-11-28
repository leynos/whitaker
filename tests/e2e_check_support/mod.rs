//! An installed-bootstrap fixture for the `whitaker check` end-to-end suites.
//!
//! `check` refuses an installation it cannot verify, and a forwarded lint run
//! goes through the installed bootstrap rather than a bare stub `cargo`. This
//! fixture installs a real (fixture) Clippy snapshot and the metadata that
//! names it, stages an empty suite library where `check` expects one, and puts
//! a stub `cargo` first on `PATH`. The stub answers the bootstrap's
//! `dylint --version` probe, records its arguments, and "builds" a driver the
//! first time it sees an empty slot, as Dylint would.
//!
//! Only the fixture API every end-to-end target needs lives here. The
//! behaviour suites keep the installation-record helpers in their own copy,
//! and a helper only one target needs lives in that target: each integration
//! target compiles this module as its own crate, so a declaration no target
//! uses would be dead code in every crate that includes it. Keeping the module
//! to the common surface means the targets need no dead-code suppression.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::{
    bootstrap::{BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath},
    crate_name::CrateName,
    resolution::SUITE_CRATE,
    stager::Stager,
    test_support::TEST_DATA_HOME_ENV,
    test_utils::{FIXTURE_TOOLCHAIN, SnapshotFixture},
};

/// The toolchain the fixture installation records.
pub const TOOLCHAIN: &str = FIXTURE_TOOLCHAIN;

/// The stub's `cargo` implementation.
const STUB_CARGO: &str = r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == dylint && "${2:-}" == --version ]]; then
    echo "cargo-dylint 6.3.0"
    exit 0
fi
# The selection suite delegates to a real Cargo: metadata answers the coverage
# listing, and the lint run becomes a real `cargo check` of the arguments
# Whitaker forwarded. Everything else keeps the recording stub's behaviour.
if [[ "${STUB_DELEGATE:-}" == "1" && -n "${STUB_REAL_CARGO:-}" ]]; then
    if [[ "${1:-}" == metadata ]]; then
        exec "${STUB_REAL_CARGO}" "$@"
    fi
    if [[ "${1:-}" == dylint ]]; then
        shift
        # Dylint's own options precede the separator; a manifest path among
        # them is also added to the cargo check that Dylint builds.
        manifest=()
        while [[ $# -gt 0 && "$1" != "--" ]]; do
            case "$1" in
                --manifest-path=*) manifest=("--manifest-path" "${1#--manifest-path=}") ;;
                --manifest-path) shift; manifest=("--manifest-path" "${1:-}") ;;
            esac
            shift
        done
        if [[ $# -gt 0 ]]; then shift; fi
        exec "${STUB_REAL_CARGO}" check ${manifest[@]+"${manifest[@]}"} "$@"
    fi
fi
{
    printf 'ARG=%s\n' "$@"
    printf 'LIB=%s\n' "${DYLINT_LIBRARY_PATH:-}"
    printf 'CLIPPY=%s\n' "${DYLINT_CLIPPY_REPO:-}"
    printf 'DRIVER=%s\n' "${DYLINT_DRIVER_PATH:-}"
} >>"${STUB_LOG}"
driver="${DYLINT_DRIVER_PATH}/${STUB_TOOLCHAIN}/dylint-driver"
if [[ ! -s "$driver" ]]; then
    mkdir -p "$(dirname "$driver")"
    printf '#!/bin/sh\n' >"$driver"
fi
if [[ -n "${STUB_STDOUT:-}" ]]; then
    printf '%s\n' "${STUB_STDOUT}"
fi
if [[ -n "${STUB_SIGNAL:-}" ]]; then
    kill -s "${STUB_SIGNAL}" "$$"
fi
exit "${STUB_EXIT:-0}"
"#;

/// An installed bootstrap, a stub `cargo`, and the environment they need.
pub struct BootstrapFixture {
    dir: TempDir,
    data_home: PathBuf,
    stub_bin: PathBuf,
}

impl BootstrapFixture {
    /// Install a snapshot, the bootstrap metadata, and the stub `cargo`.
    ///
    /// # Errors
    ///
    /// Returns a description of the first fixture step that failed.
    pub fn new() -> Result<Self, String> {
        let dir = TempDir::new()
            .map_err(|error| format!("temporary directory should be created: {error}"))?;
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| format!("temporary path should be UTF-8: {}", path.display()))?;
        let data_home = base.join("xdg-data");
        let layout = BootstrapLayout::at(&data_home.join("whitaker-bootstrap"));
        let snapshot = SnapshotFixture::new()
            .map_err(|error| format!("fixture snapshot should be built: {error}"))?;
        snapshot
            .install(&layout.clippy_root())
            .map_err(|error| format!("fixture snapshot should be installed: {error}"))?;
        let library_dir = data_home.join("whitaker/lints/lib");
        std::fs::create_dir_all(&library_dir)
            .map_err(|error| format!("library directory should be created: {error}"))?;
        // A real installation leaves the staged suite in that directory, and
        // `check` refuses a recorded installation that does not hold it.
        let suite_filename = Stager::new(Utf8PathBuf::from("unused"), TOOLCHAIN)
            .staged_filename(&CrateName::from(SUITE_CRATE));
        std::fs::write(library_dir.join(suite_filename), b"")
            .map_err(|error| format!("staged suite should be written: {error}"))?;
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: snapshot.commit().to_owned(),
            toolchain: TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_home),
            suite_version: None,
        }
        .write(&layout.metadata_path())
        .map_err(|error| format!("bootstrap metadata should be written: {error}"))?;
        let stub_bin = dir.path().join("bin");
        write_executable(&stub_bin.join("cargo"), STUB_CARGO)?;
        Ok(Self {
            dir,
            data_home: data_home.into_std_path_buf(),
            stub_bin,
        })
    }

    /// Build a command for the root binary, pointed at this installation.
    ///
    /// `PATH` keeps the real entries after the stub, so the script's own
    /// `/usr/bin/env bash` and any Git lookup still resolve.
    ///
    /// Debug test builds also name the fixture's project-data directory
    /// through the installer's test override, the only seam that covers a
    /// platform whose native data directory ignores `XDG_DATA_HOME`. Release
    /// test builds ignore that override and resolve the native directory, but
    /// `HOME` and `XDG_DATA_HOME` still keep it inside this fixture's
    /// temporary tree on Unix.
    ///
    /// # Errors
    ///
    /// Returns an error when the inherited `PATH` cannot be joined.
    pub fn command(&self) -> Result<Command, String> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
        command
            .env("XDG_DATA_HOME", &self.data_home)
            .env("HOME", self.dir.path().join("home"))
            .env("PATH", self.path_with_stub()?)
            .env("STUB_LOG", self.log_path())
            .env("STUB_TOOLCHAIN", TOOLCHAIN)
            .env("STUB_EXIT", "23")
            .env_remove("STUB_SIGNAL")
            .env_remove("STUB_STDOUT")
            .env_remove("DYLINT_LIBRARY_PATH")
            .env_remove("DYLINT_CLIPPY_REPO")
            .env_remove("DYLINT_DRIVER_PATH");
        if cfg!(debug_assertions) {
            command.env(TEST_DATA_HOME_ENV, self.data_root());
        }
        Ok(command)
    }

    /// Name the project-data directory the child resolves through its data home.
    pub fn data_root(&self) -> PathBuf { self.data_home.join("whitaker") }

    /// Name the log the stub `cargo` appends to.
    fn log_path(&self) -> PathBuf { self.dir.path().join("stub.log") }

    /// Build the inherited `PATH` with the stub directory first.
    ///
    /// # Errors
    ///
    /// Returns an error when the entries cannot be joined.
    fn path_with_stub(&self) -> Result<OsString, String> {
        let mut entries = vec![self.stub_bin.clone()];
        entries.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(entries)
            .map_err(|error| format!("fixture PATH should be valid: {error}"))
    }
}

/// Write an executable script, creating its directory.
///
/// # Errors
///
/// Returns a description of the first filesystem step that failed.
fn write_executable(path: &Path, contents: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let parent = path
        .parent()
        .ok_or_else(|| format!("script should have a parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("script directory should be created: {error}"))?;
    std::fs::write(path, contents).map_err(|error| format!("script should be written: {error}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("script should be executable: {error}"))
}
