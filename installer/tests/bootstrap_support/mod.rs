//! An installed Whitaker bootstrap in a temporary data root, with a stub
//! `cargo` that records what `whitaker-installer run` hands it.
//!
//! The snapshot is a real Git generation installed through the library, so
//! `run` performs its real checks; only `cargo dylint` is replaced. The stub
//! "builds" a driver the first time it sees an empty slot, counting builds in
//! a log, so tests can tell a cold run from a warm one.

use camino::{Utf8Path, Utf8PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::bootstrap::{
    BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath,
};

use whitaker_installer::test_utils::{
    FIXTURE_TOOLCHAIN as TOOLCHAIN_PIN, SnapshotFixture, fixture_git,
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
    /// Owns every path below.
    pub dir: TempDir,
    /// The current `XDG_DATA_HOME`.
    pub data_root: Utf8PathBuf,
    /// The installed snapshot commit.
    pub commit: String,
}

impl Installation {
    /// Install a snapshot, lint directory, metadata, and stub `cargo`.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("create temp dir");
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8");
        let data_root = base.join("data");
        let layout = BootstrapLayout::at(&data_root.join("whitaker-bootstrap"));
        let commit = install_snapshot(&layout.clippy_root());
        let library_dir = data_root.join("whitaker/lints/lib");
        std::fs::create_dir_all(&library_dir).expect("create library dir");
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: commit.clone(),
            toolchain: TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_root),
        }
        .write(&layout.metadata_path())
        .expect("write metadata");
        write_executable(&base.join("bin/cargo"), STUB_CARGO);
        Self {
            dir,
            data_root,
            commit,
        }
    }

    /// The temporary base directory.
    pub fn base(&self) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(self.dir.path().to_path_buf()).expect("UTF-8")
    }

    /// The bootstrap layout under the current data root.
    pub fn layout(&self) -> BootstrapLayout {
        BootstrapLayout::at(&self.data_root.join("whitaker-bootstrap"))
    }

    /// Move the whole data root, as relocating a home directory would.
    pub fn relocate(&mut self, to: &str) {
        let destination = self.base().join(to);
        std::fs::rename(&self.data_root, &destination).expect("move data root");
        self.data_root = destination;
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
    pub fn run(&self, args: &[&str]) -> Output {
        self.run_command(args)
            .output()
            .expect("run whitaker-installer")
    }

    /// How many cold driver builds the stub has performed.
    pub fn builds(&self) -> usize {
        std::fs::read_to_string(self.base().join("stub.builds"))
            .map(|log| log.lines().count())
            .unwrap_or(0)
    }

    /// The environment lines the stub recorded, in order.
    pub fn recorded(&self) -> Vec<String> {
        std::fs::read_to_string(self.base().join("stub.env"))
            .map(|log| log.lines().map(ToOwned::to_owned).collect())
            .unwrap_or_default()
    }
}

/// Install the shared fixture snapshot under `clippy_root`; return its commit.
fn install_snapshot(clippy_root: &Utf8Path) -> String {
    let fixture = SnapshotFixture::new().expect("build the fixture snapshot");
    fixture
        .install(clippy_root)
        .expect("install the fixture snapshot");
    fixture.commit().to_owned()
}

/// Run fixture Git, failing the test on error.
pub fn git(dir: &Utf8Path, args: &[&str]) -> String {
    fixture_git(dir, args).expect("fixture git")
}

/// Write an executable script.
pub fn write_executable(path: &Utf8Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().expect("script has a parent")).expect("create dir");
    std::fs::write(path, contents).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}
