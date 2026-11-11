//! Tests for bootstrap selection and child-process behaviour.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::{dirs::BaseDirs, orchestration::run_bootstrap_command_with_dirs};

use super::{forward_source, is_installed};
use crate::adapters::telemetry::ForwardSource;

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
    fn new(temporary: &TempDir) -> Self { Self::at(temporary.path().to_path_buf()) }

    /// Anchor the platform directories at an arbitrary root.
    fn at(root: PathBuf) -> Self { Self { root } }
}

impl BaseDirs for TempBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.root.join("whitaker")) }
}

/// A host with no bootstrap record has nothing to run from.
#[test]
fn absent_bootstrap_metadata_reports_no_installation() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::new(&temporary);

    assert!(
        !is_installed(&dirs),
        "no bootstrap.json means there is nothing to run from"
    );
}

/// A recorded bootstrap, even a broken one, is an installation to run from.
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

/// No record means no bootstrap, so the forward runs Dylint directly.
#[test]
fn an_absent_record_forwards_directly() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::new(&temporary);

    assert_eq!(
        forward_source(Some(&dirs)),
        ForwardSource::Direct,
        "a host with no record has no bootstrap to run through"
    );
}

/// A bootstrap directory with no record in it is not something to run from.
#[test]
fn a_record_directory_without_metadata_forwards_through_the_bootstrap() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    // A bootstrap directory with no record in it is a real installation that
    // lost its metadata: the directory exists, the record does not.
    std::fs::create_dir_all(data_home.join("whitaker-bootstrap"))
        .expect("the bootstrap directory should be created");
    let dirs = TempBaseDirs::new(&temporary);

    assert_eq!(
        forward_source(Some(&dirs)),
        ForwardSource::Direct,
        "a bootstrap directory with no record is not something to run from"
    );
}

/// A record that cannot be inspected must route through the bootstrap, so the
/// run reports the defect instead of silently falling back to a direct run.
#[test]
fn an_uninspectable_record_forwards_through_the_bootstrap() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    // A NUL byte makes every platform refuse the record path with
    // `InvalidInput`, which is the branch a permission error on the data
    // directory takes in the field. A file standing in for the bootstrap
    // directory would not do: Windows reports a path through a file as
    // `NotFound`, which is genuine absence, and a directory *at* the record
    // path inspects successfully.
    let dirs = TempBaseDirs::at(temporary.path().join("uninspectable\0root"));

    assert_eq!(
        forward_source(Some(&dirs)),
        ForwardSource::Bootstrap,
        "a record that cannot be inspected must still route through the bootstrap, so the run \
         reports the defect instead of silently falling back"
    );
}

/// A host that cannot name its data directory has no bootstrap to run through.
#[test]
fn no_platform_directories_forwards_directly() {
    assert_eq!(
        forward_source(None),
        ForwardSource::Direct,
        "a host that cannot name its data directory has no bootstrap to run through"
    );
}

/// The route the selection returns is the one that reports a broken install.
///
/// The route tests and the diagnostic test each pin one half of the forward.
/// This drives both halves in one invocation, so a change that selected the
/// bootstrap but dispatched elsewhere, or reported nothing, would fail here.
#[test]
fn an_uninspectable_record_forwards_and_reports_the_defect() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::at(temporary.path().join("uninspectable\0root"));
    let mut stderr = Vec::new();

    assert_eq!(
        forward_source(Some(&dirs)),
        ForwardSource::Bootstrap,
        "the defect must be reported by the bootstrap, not hidden by a direct run"
    );
    let code = run_bootstrap_command_with_dirs(&dirs, &[], None, &mut stderr);

    assert_eq!(code, 1, "a failed bootstrap run must fail the invocation");
    let report = String::from_utf8(stderr).expect("the report should be UTF-8");
    assert!(
        report.starts_with("whitaker: "),
        "the diagnostic should name the command and the defect, got: {report}"
    );
}
