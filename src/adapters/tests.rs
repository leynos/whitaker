//! Tests for bootstrap selection and child-process behaviour.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::dirs::BaseDirs;

use super::is_installed;

/// Convert a temporary path to the UTF-8 form the installer uses.
fn utf8_path(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .expect("temporary test paths should be valid UTF-8")
}

/// A [`BaseDirs`] implementation rooted at a temporary directory.
///
/// The record tests need a data directory they own. Redirecting the
/// platform directory through an environment variable works on Unix only,
/// so the directory is injected instead and the tests behave the same on
/// every platform. The default `bootstrap_dir` places the bootstrap beside
/// `whitaker/`, mirroring the real layout.
struct TempBaseDirs {
    root: PathBuf,
}

impl TempBaseDirs {
    /// Anchor the platform directories inside `temporary`'s tree.
    fn new(temporary: &TempDir) -> Self {
        Self {
            root: temporary.path().to_path_buf(),
        }
    }
}

impl BaseDirs for TempBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.root.join("whitaker")) }
}

#[test]
fn absent_bootstrap_metadata_reports_no_installation() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::new(&temporary);

    assert!(
        !is_installed(&dirs),
        "no bootstrap.json means there is nothing to run from"
    );
}

#[test]
fn recorded_bootstrap_metadata_reports_an_installation() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    // The metadata's contents are irrelevant here: the selection asks only
    // whether a record exists, and a broken record must still be reported
    // by the bootstrap rather than silently bypassed.
    let bootstrap_dir = data_home.join("whitaker-bootstrap");
    std::fs::create_dir_all(&bootstrap_dir)
        .expect("bootstrap directory should be created inside the temporary directory");
    std::fs::write(bootstrap_dir.join("bootstrap.json"), b"{}")
        .expect("bootstrap metadata should be written");
    let dirs = TempBaseDirs::new(&temporary);

    assert!(is_installed(&dirs), "a recorded bootstrap must be used");
}
