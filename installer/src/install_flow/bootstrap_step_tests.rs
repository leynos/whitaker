//! The dry-run bootstrap plan's view of the snapshot generation.

use super::*;
use std::path::PathBuf;
use tempfile::TempDir;
use whitaker_installer::bootstrap::BootstrapError;

/// Platform directories rooted in a temporary directory.
struct TempDirs(PathBuf);

impl BaseDirs for TempDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }
    fn bin_dir(&self) -> Option<PathBuf> {
        None
    }
    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        Some(self.0.join("whitaker"))
    }
}

#[test]
fn the_plan_names_an_absent_snapshot_as_to_be_installed() {
    let temp = TempDir::new().expect("create temp dir");
    let mut stderr = Vec::new();

    report_bootstrap_plan(&TempDirs(temp.path().to_path_buf()), &mut stderr)
        .expect("report the plan");

    let report = String::from_utf8(stderr).expect("UTF-8 report");
    assert!(
        report.contains("Clippy snapshot state: to be installed"),
        "{report}"
    );
}

/// Regression: `is_dir` once read an uninspectable generation as absent, so a
/// dry run promised an install that would then fail.
#[test]
fn the_plan_reports_a_snapshot_it_cannot_inspect() {
    let temp = TempDir::new().expect("create temp dir");
    let bootstrap = temp.path().join("whitaker-bootstrap");
    std::fs::create_dir_all(&bootstrap).expect("create bootstrap dir");
    // A file where the generations directory belongs.
    std::fs::write(bootstrap.join("clippy"), b"not a directory").expect("write file");
    let mut stderr = Vec::new();

    let error = report_bootstrap_plan(&TempDirs(temp.path().to_path_buf()), &mut stderr)
        .expect_err("an uninspectable generation is an error");

    assert!(
        matches!(error, InstallerError::Bootstrap(BootstrapError::Io { .. })),
        "{error:?}"
    );
}
