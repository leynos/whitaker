//! The bootstrap plan's view of the snapshot, and the diagnostic a failed
//! bootstrap run reports.

use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::bootstrap::BootstrapError;

/// Platform directories rooted in a temporary directory.
struct TempDirs(PathBuf);

impl BaseDirs for TempDirs {
    fn home(&self) -> Option<PathBuf> { None }
    fn executables(&self) -> Option<PathBuf> { None }
    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.0.join("whitaker")) }
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
    // A NUL byte makes every platform refuse the path with `InvalidInput`.
    // A file standing in for a directory would not do: Windows reports a
    // path through a file as `NotFound`, which is genuine absence.
    let dirs = TempDirs(temp.path().join("uninspectable\0root"));
    let mut stderr = Vec::new();

    let error = report_bootstrap_plan(&dirs, &mut stderr)
        .expect_err("an uninspectable generation is an error");

    assert!(
        matches!(error, InstallerError::Bootstrap(BootstrapError::Io { .. })),
        "{error:?}"
    );
}

/// Regression: the forward path must report a broken installation, not exit
/// silently. `run_bootstrap_with_dirs` is the function the forward route calls
/// once it has chosen the bootstrap, so this pins the diagnostic at that
/// boundary.
#[test]
fn a_bootstrap_that_cannot_run_reports_the_defect() {
    let temp = TempDir::new().expect("create temp dir");
    // A NUL byte makes every platform refuse the record path, which is the
    // uninspectable installation the forward route must report.
    let dirs = TempDirs(temp.path().join("uninspectable\0root"));
    let mut stderr = Vec::new();

    let code = run_bootstrap_with_dirs(&dirs, &[], None, &mut stderr);

    assert_eq!(code, 1, "a failed bootstrap run must fail the invocation");
    let report = String::from_utf8(stderr).expect("UTF-8 report");
    assert!(
        report.starts_with("whitaker: "),
        "the diagnostic should name the command and the defect, got: {report}"
    );
}
