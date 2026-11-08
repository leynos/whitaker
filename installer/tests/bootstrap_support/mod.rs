//! An installed Whitaker bootstrap in a temporary data root, with a stub
//! `cargo` that records what `whitaker-installer run` hands it.
//!
//! The snapshot is a real Git generation installed through the library, so
//! `run` performs its real checks; only `cargo dylint` is replaced. The stub
//! "builds" a driver the first time it sees an empty slot, counting builds in
//! a log, so tests can tell a cold run from a warm one.

use std::process::{Command, Output};

use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use whitaker_installer::{
    bootstrap::{BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath},
    test_utils::{FIXTURE_TOOLCHAIN as TOOLCHAIN_PIN, SnapshotFixture, fixture_git},
};

/// The nightly the fixture installation records.
pub const TOOLCHAIN: &str = TOOLCHAIN_PIN;

/// The stub's `cargo` implementation.
const STUB_CARGO: &str = r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == dylint && "${2:-}" == --version ]]; then
    echo "cargo-dylint ${STUB_DYLINT_VERSION:-6.3.0}"
    exit 0
fi
{
    printf 'LIB=%s\n' "${DYLINT_LIBRARY_PATH:-}"
    printf 'CLIPPY=%s\n' "${DYLINT_CLIPPY_REPO:-}"
    printf 'DRIVER=%s\n' "${DYLINT_DRIVER_PATH:-}"
    printf 'WRAPPER=%s\n' "${RUSTC_WRAPPER:-}"
    printf 'ARG=%s\n' "$@"
} >>"${STUB_LOG}.env"
driver_dir="${DYLINT_DRIVER_PATH}/${STUB_TOOLCHAIN}"
# STUB_REPLACE_DRIVER rebuilds a ready driver with identical bytes, as Dylint
# does when its version probe of a cached driver fails. It says nothing.
if [[ "${STUB_REPLACE_DRIVER:-}" == 1 && -s "${driver_dir}/dylint-driver" ]]; then
    printf '#!/bin/sh\n' >"${driver_dir}/dylint-driver.partial"
    mv "${driver_dir}/dylint-driver.partial" "${driver_dir}/dylint-driver"
fi
if [[ ! -s "${driver_dir}/dylint-driver" ]]; then
    # STUB_FAIL_BUILD fails the build without leaving a driver. Failed
    # attempts are logged apart so `builds` counts only completed builds.
    if [[ "${STUB_FAIL_BUILD:-}" == 1 ]]; then
        echo failed >>"${STUB_LOG}.failed"
        exit 1
    fi
    echo build >>"${STUB_LOG}.builds"
    sleep 1
    mkdir -p "${driver_dir}"
    printf '#!/bin/sh\n' >"${driver_dir}/dylint-driver.partial"
    mv "${driver_dir}/dylint-driver.partial" "${driver_dir}/dylint-driver"
fi
exit "${STUB_EXIT:-0}"
"#;

/// A temporary installation.
pub struct Installation {
    /// Owns every path below; dropped with the installation.
    ///
    /// The field is never read directly, but it must stay alive for as long as
    /// the installation so the temporary directory is not removed early.
    pub _dir: TempDir,
    /// The current `XDG_DATA_HOME`.
    pub data_root: Utf8PathBuf,
    /// The installed snapshot commit.
    pub commit: String,
    /// The temporary base directory, resolved once at construction.
    base: Utf8PathBuf,
}

impl Installation {
    /// Install a snapshot, lint directory, metadata, and stub `cargo`.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory, snapshot, metadata, or
    /// stub `cargo` cannot be created.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| format!("temporary directory is not UTF-8: {}", path.display()))?;
        let data_root = base.join("data");
        let layout = BootstrapLayout::at(&data_root.join("whitaker-bootstrap"));
        let commit = install_snapshot(&layout.clippy_root())?;
        let library_dir = data_root.join("whitaker/lints/lib");
        std::fs::create_dir_all(&library_dir)?;
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: commit.clone(),
            toolchain: TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_root),
        }
        .write(&layout.metadata_path())?;
        write_executable(&base.join("bin/cargo"), STUB_CARGO)?;
        Ok(Self {
            _dir: dir,
            data_root,
            commit,
            base,
        })
    }

    /// The temporary base directory.
    pub fn base(&self) -> Utf8PathBuf { self.base.clone() }

    /// The bootstrap layout under the current data root.
    pub fn layout(&self) -> BootstrapLayout {
        BootstrapLayout::at(&self.data_root.join("whitaker-bootstrap"))
    }

    /// Move the whole data root, as relocating a home directory would.
    ///
    /// # Errors
    ///
    /// Returns an error if the data root cannot be moved.
    pub fn relocate(&mut self, to: &str) -> Result<(), Box<dyn std::error::Error>> {
        let destination = self.base().join(to);
        std::fs::rename(&self.data_root, &destination)?;
        self.data_root = destination;
        Ok(())
    }

    /// A `whitaker-installer run -- <args>` command against this installation.
    pub fn run_command(&self, args: &[&str]) -> Command {
        let base = self.base();
        let path = format!(
            "{}:{}",
            base.join("bin"),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
        command
            .arg("run")
            .arg("--")
            .args(args)
            .env("XDG_DATA_HOME", &self.data_root)
            .env("HOME", base.join("home"))
            .env("PATH", path)
            .env("STUB_LOG", base.join("stub"))
            // Stale values the bootstrap must overwrite rather than trust.
            .env("DYLINT_CLIPPY_REPO", "/stale/clippy")
            .env("DYLINT_DRIVER_PATH", "/stale/drivers")
            .env("DYLINT_LIBRARY_PATH", "/stale/lib")
            // The stub builds where `DriverSlot` looks: under this toolchain.
            .env("STUB_TOOLCHAIN", TOOLCHAIN);
        command
    }

    /// Run with `args` and wait for the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the installer cannot be launched or awaited.
    pub fn run(&self, args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
        Ok(self.run_command(args).output()?)
    }

    /// How many cold driver builds the stub has performed.
    pub fn builds(&self) -> usize {
        std::fs::read_to_string(self.base().join("stub.builds"))
            .map_or(0, |log| log.lines().count())
    }

    /// The environment lines the stub recorded, in order.
    pub fn recorded(&self) -> Vec<String> {
        std::fs::read_to_string(self.base().join("stub.env"))
            .map(|log| log.lines().map(ToOwned::to_owned).collect())
            .unwrap_or_default()
    }
}

/// Install the shared fixture snapshot under `clippy_root`; return its commit.
fn install_snapshot(clippy_root: &Utf8Path) -> Result<String, Box<dyn std::error::Error>> {
    let fixture = SnapshotFixture::new()?;
    fixture.install(clippy_root)?;
    Ok(fixture.commit().to_owned())
}

/// Run fixture Git, returning its output.
///
/// # Errors
///
/// Returns an error if Git cannot be run or exits unsuccessfully.
pub fn git(dir: &Utf8Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    Ok(fixture_git(dir, args)?)
}

/// Write an executable script.
///
/// # Errors
///
/// Returns an error if the parent directory, script, or permissions cannot be
/// written.
pub fn write_executable(path: &Utf8Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let parent = path.parent().ok_or("script path has no parent directory")?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, contents)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}
