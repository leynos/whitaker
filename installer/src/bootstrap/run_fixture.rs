//! A complete installation and a stub `cargo`, shared by the bootstrap tests.
//!
//! The query tests and the promoted-API tests both need an installation that
//! is genuinely ready: the same metadata, snapshot and lint directory the
//! installer writes. Building that fixture twice would let the two suites
//! drift, and a readiness check that passes one but not the other would hide
//! the difference.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use tempfile::TempDir;

use super::{BootstrapLayout, BootstrapMetadata};
use crate::{
    bootstrap::{BOOTSTRAP_SCHEMA, StoredPath},
    dirs::BaseDirs,
    test_utils::{FIXTURE_TOOLCHAIN, SnapshotFixture},
};

/// A [`BaseDirs`] rooted at one fixture's temporary data directory.
///
/// Only [`BaseDirs::whitaker_data`] matters here: the bootstrap directory is
/// derived from its parent, which is exactly how the production directories
/// place it.
pub(super) struct RootedDirs {
    data: PathBuf,
}

impl BaseDirs for RootedDirs {
    /// Report that the fake leaves the home directory unset.
    fn home(&self) -> Option<PathBuf> { None }

    /// Report that the fake leaves the executables directory unset.
    fn executables(&self) -> Option<PathBuf> { None }

    /// Return the fixture's temporary data directory.
    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}

/// Render a stub `cargo` that reports Dylint 6.3.0 and, when `builds` is set,
/// writes a driver into the exported slot if none exists, as Dylint would.
pub(super) fn stub_cargo(builds: bool) -> String {
    format!(
        r#"#!/usr/bin/env bash
if [[ "$1" == dylint && "$2" == --version ]]; then
    echo "cargo-dylint 6.3.0"
    exit 0
fi
driver="$DYLINT_DRIVER_PATH/{FIXTURE_TOOLCHAIN}/dylint-driver"
# Like Dylint, build only when no driver exists yet.
if [[ {builds} == yes && ! -s "$driver" ]]; then
    mkdir -p "$(dirname "$driver")"
    printf 'driver' >"$driver"
fi
"#,
        builds = if builds { "yes" } else { "no" },
    )
}

/// A complete installation in a temporary data root.
pub(super) struct Installed {
    pub(super) _dir: TempDir,
    pub(super) _fixture: SnapshotFixture,
    pub(super) layout: BootstrapLayout,
    pub(super) library_dir: Utf8PathBuf,
    pub(super) commit: String,
}

impl Installed {
    /// Install a snapshot, a lint directory, and the bootstrap metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory, snapshot fixture, or
    /// metadata cannot be created.
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| format!("temp path is not UTF-8: {}", path.display()))?;
        let data_root = base.join("data");
        let layout = BootstrapLayout::at(&data_root.join("whitaker-bootstrap"));
        let fixture = SnapshotFixture::new()?;
        fixture.install(&layout.clippy_root())?;
        let commit = fixture.commit().to_owned();
        let library_dir = data_root.join("whitaker/lints/lib");
        std::fs::create_dir_all(&library_dir)?;
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: commit.clone(),
            toolchain: FIXTURE_TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_root),
            suite_version: None,
        }
        .write(&layout.metadata_path())?;
        Ok(Self {
            _dir: dir,
            _fixture: fixture,
            layout,
            library_dir,
            commit,
        })
    }

    /// Write a stub `cargo` at the data root and return its path.
    ///
    /// The path must not run through the recorded library directory: the
    /// explicit-override test removes that directory, and spawning a program
    /// through a deleted path component fails with `ENOENT` even when the
    /// lexical destination still exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the stub cannot be written or made executable.
    pub(super) fn cargo(&self, builds: bool) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let name = if builds { "cargo-builds" } else { "cargo-idle" };
        let path = self.layout.data_root().join(name);
        std::fs::write(&path, stub_cargo(builds))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    /// Resolve the driver slot this installation selects for `cargo dylint`.
    pub(super) fn slot_dir(&self) -> Utf8PathBuf { self.layout.driver_dir("6.3.0", &self.commit) }

    /// Construct the platform directories that resolve to this installation.
    pub(super) fn dirs(&self) -> RootedDirs {
        RootedDirs {
            data: self.layout.data_root().join("whitaker").into(),
        }
    }
}
