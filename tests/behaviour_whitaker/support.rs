//! A root-CLI fixture: an installed bootstrap plus a stub `cargo`.
//!
//! A forwarded lint run now goes through the installed bootstrap, so a test
//! that only stubbed `cargo` would fail the installation checks. This fixture
//! installs a real (fixture) Clippy snapshot and the metadata that names it,
//! then records what the stub `cargo` is handed.
//!
//! The stub answers the bootstrap's `dylint --version` probe, appends its
//! arguments and `DYLINT_LIBRARY_PATH` to a log, and "builds" a driver the
//! first time it sees an empty slot, as Dylint would.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::{
    bootstrap::{BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath},
    dirs::BaseDirs,
    installation_record::record_installation,
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
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: snapshot.commit().to_owned(),
            toolchain: TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_home),
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

    /// A command for the root binary, pointed at this installation.
    ///
    /// `PATH` keeps the real entries after the stub, so the script's own
    /// `/usr/bin/env bash` and any Git lookup still resolve.
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
            .env_remove("DYLINT_LIBRARY_PATH")
            .env_remove("DYLINT_CLIPPY_REPO")
            .env_remove("DYLINT_DRIVER_PATH");
        Ok(command)
    }

    /// Record another toolchain's staging directory, as an install would.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation record cannot be written.
    pub fn record_toolchain(
        &self,
        toolchain: &str,
        staging: &camino::Utf8Path,
    ) -> Result<(), String> {
        record_installation(&self.dirs(), toolchain, staging)
            .map_err(|error| format!("staging directory should be recorded: {error}"))
    }

    /// The executable's own output directory, for tests that create files.
    pub fn root(&self) -> &Path { self.dir.path() }

    /// Every line the stub appended, in order.
    pub fn recorded(&self) -> Vec<String> {
        std::fs::read_to_string(self.log_path()).map_or_else(
            |_| Vec::new(),
            |log| log.lines().map(ToOwned::to_owned).collect(),
        )
    }

    /// The arguments the stub received, including the `dylint` subcommand.
    pub fn forwarded_arguments(&self) -> Vec<String> {
        self.recorded()
            .into_iter()
            .filter_map(|line| line.strip_prefix("ARG=").map(ToOwned::to_owned))
            .collect()
    }

    /// The last `DYLINT_LIBRARY_PATH` the stub was given.
    pub fn library_path(&self) -> Option<String> { self.last_recorded("LIB=") }

    /// The last `DYLINT_CLIPPY_REPO` the stub was given.
    pub fn clippy_repo(&self) -> Option<String> { self.last_recorded("CLIPPY=") }

    /// The last `DYLINT_DRIVER_PATH` the stub was given.
    pub fn driver_path(&self) -> Option<String> { self.last_recorded("DRIVER=") }

    /// The last value the stub recorded under `prefix`.
    fn last_recorded(&self, prefix: &str) -> Option<String> {
        self.recorded()
            .into_iter()
            .rev()
            .find_map(|line| line.strip_prefix(prefix).map(ToOwned::to_owned))
    }

    /// The data directories the root binary resolves through `XDG_DATA_HOME`.
    fn dirs(&self) -> FixtureDirs {
        FixtureDirs {
            data: self.data_home.join("whitaker"),
        }
    }

    fn log_path(&self) -> PathBuf { self.dir.path().join("stub.log") }

    fn path_with_stub(&self) -> Result<OsString, String> {
        let mut entries = vec![self.stub_bin.clone()];
        entries.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(entries)
            .map_err(|error| format!("fixture PATH should be valid: {error}"))
    }
}

/// The minimum [`BaseDirs`] implementation for recording a toolchain.
struct FixtureDirs {
    data: PathBuf,
}

impl BaseDirs for FixtureDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
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
