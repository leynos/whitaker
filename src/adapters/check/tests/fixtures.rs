//! Platform directories a test can point at a record without touching the host.
//!
//! The bootstrap layout is derived from these directories, so a test can put
//! an installation record exactly where a real one lives, inside a temporary
//! root that is removed when the test ends.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::{
    bootstrap::{BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath},
    dirs::BaseDirs,
};

/// Platform directories rooted in one temporary directory.
pub(super) struct TempDirs {
    root: PathBuf,
}

impl TempDirs {
    /// Root every platform directory under `root`.
    fn new(root: &TempDir) -> Self {
        Self {
            root: root.path().to_owned(),
        }
    }

    /// The bootstrap layout a repair resolves for these directories.
    pub(super) fn layout(&self) -> BootstrapLayout {
        BootstrapLayout::from_dirs(self).expect("a temporary root names a bootstrap directory")
    }
}

impl BaseDirs for TempDirs {
    fn home(&self) -> Option<PathBuf> { Some(self.root.join("home")) }

    fn executables(&self) -> Option<PathBuf> { Some(self.root.join("home/.local/bin")) }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.root.join("data/whitaker")) }
}

/// Temporary platform directories, and the temporary directory behind them.
///
/// The directory is returned so the caller keeps it alive: dropping it would
/// remove the record the test just wrote.
pub(super) fn temp_dirs() -> (TempDir, TempDirs) {
    let root = TempDir::new().expect("a temporary directory is available");
    let dirs = TempDirs::new(&root);
    (root, dirs)
}

/// An installation record, carrying `pin` when one is given.
pub(super) fn record(pin: Option<&str>) -> BootstrapMetadata {
    BootstrapMetadata {
        schema_version: BOOTSTRAP_SCHEMA,
        dylint_version: "6.3.0".to_owned(),
        snapshot_commit: "e4608a25".to_owned(),
        toolchain: "nightly-2026-05-28".to_owned(),
        library_dir: StoredPath::DataRootRelative(Utf8PathBuf::from("whitaker/lints")),
        suite_version: pin.map(str::to_owned),
    }
}
