//! Tests for the staging-record failure policy.

use std::{path::PathBuf, time::Instant};

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use tempfile::TempDir;

use super::{
    FinishInstallContext,
    INSTALLATION_RECORD_TOTAL,
    StagingRecord,
    finish_install_and_record_metrics,
    record_installed_path,
};
use crate::{
    bootstrap::BootstrapLayout,
    cli::InstallArgs,
    dirs::{BaseDirs, MockBaseDirs},
    error::InstallerError,
    install_flow::bootstrap_step::InstalledSnapshot,
    install_metrics::InstallMode,
    wrapper::WrapperScripts,
};

/// One recorded metric, as a debugging snapshot lists it.
type Recorded = (
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
);

/// Return whether `recorded` holds the record-write counter with `label`.
fn counted_once(recorded: &[Recorded], label: (&str, &str)) -> bool {
    recorded.iter().any(|(key, _, _, value)| {
        key.key().name() == INSTALLATION_RECORD_TOTAL
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == label.0 && entry.value() == label.1)
            && matches!(value, DebugValue::Counter(1))
    })
}

/// A directory provider with no Whitaker data directory.
///
/// Recording needs a data directory, so this is the smallest way to make
/// `record_installation` fail without touching the host's filesystem.
struct NoDataDirectory;

impl BaseDirs for NoDataDirectory {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { None }
}

/// A staging directory that exists, so only the record path can fail.
fn staging_directory() -> (TempDir, Utf8PathBuf) {
    let directory = TempDir::new().expect("a temporary staging directory should be created");
    let path = Utf8PathBuf::try_from(directory.path().to_path_buf())
        .expect("a temporary path should be UTF-8");
    (directory, path)
}

#[test]
fn a_root_install_treats_a_record_failure_as_fatal() {
    let (_staging, staging_path) = staging_directory();
    let record = StagingRecord {
        dirs: &NoDataDirectory,
        toolchain: "nightly-2026-05-28",
        staging_path: &staging_path,
        wrapper_scripts: WrapperScripts::None,
    };
    let mut stderr = Vec::new();
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let error =
        metrics::with_local_recorder(&recorder, || record_installed_path(&record, &mut stderr))
            .expect_err("the root CLI cannot forward without a recorded path");

    assert!(
        matches!(error, InstallerError::InstallationRecord { .. }),
        "expected an installation-record error, got {error:?}"
    );
    assert!(
        stderr.is_empty(),
        "a fatal record failure should not also warn: {}",
        String::from_utf8_lossy(&stderr)
    );
    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, ("installer", "root")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, ("outcome", "failed")),
        "{captured:#?}"
    );
}

#[test]
fn a_legacy_install_warns_about_a_record_failure_and_continues() {
    let (_staging, staging_path) = staging_directory();
    let record = StagingRecord {
        dirs: &NoDataDirectory,
        toolchain: "nightly-2026-05-28",
        staging_path: &staging_path,
        wrapper_scripts: WrapperScripts::Both,
    };
    let mut stderr = Vec::new();
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || record_installed_path(&record, &mut stderr))
        .expect("the legacy installer can still run from inside the repository");

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, ("installer", "legacy")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, ("outcome", "failed")),
        "{captured:#?}"
    );
    let warning = String::from_utf8(stderr).expect("the warning should be UTF-8");
    assert!(
        warning.starts_with("Warning: "),
        "the legacy installer should warn, got {warning:?}"
    );
    assert!(
        warning.contains("failed to record installed lint path"),
        "the warning should name the failure, got {warning:?}"
    );
}
/// A directory provider rooted in a temporary tree.
fn mock_dirs(root: &std::path::Path) -> MockBaseDirs {
    let mut dirs = MockBaseDirs::new();
    dirs.expect_whitaker_data()
        .return_const(Some(root.join("data")));
    dirs.expect_bootstrap_dir()
        .return_const(Some(root.join("whitaker-bootstrap")));
    dirs.expect_executables()
        .return_const(Some(root.join("bin")));
    dirs
}

/// A snapshot whose layout points into `root`, so no test writes to a real
/// data directory.
fn snapshot_in(root: &std::path::Path) -> InstalledSnapshot {
    let bootstrap = root.join("whitaker-bootstrap");
    let layout = Utf8Path::from_path(&bootstrap).expect("a temporary path should be UTF-8");
    InstalledSnapshot::for_test(BootstrapLayout::at(layout))
}

/// The finalizer must not swallow the record error the root CLI depends on.
#[test]
fn a_root_install_fails_through_the_finalizer() {
    let (_staging, staging_path) = staging_directory();
    let args = InstallArgs::parse_from(["whitaker-installer"]);
    let snapshot = InstalledSnapshot::for_test(BootstrapLayout::at(Utf8Path::new(
        "/data/whitaker-bootstrap",
    )));
    let context = FinishInstallContext {
        args: &args,
        dirs: &NoDataDirectory,
        snapshot: &snapshot,
        toolchain: "nightly-2026-05-28",
        staging_path: &staging_path,
        wrapper_scripts: WrapperScripts::None,
        install_mode: InstallMode::Build,
        install_started: Instant::now(),
    };
    let mut stderr = Vec::new();

    let error = finish_install_and_record_metrics(&context, &mut stderr)
        .expect_err("the root CLI cannot finish without a recorded path");

    assert!(
        matches!(error, InstallerError::InstallationRecord { .. }),
        "expected an installation-record error, got {error:?}"
    );
    assert!(
        stderr.is_empty(),
        "a fatal record failure should not also warn: {}",
        String::from_utf8_lossy(&stderr)
    );
}

/// The legacy installer must warn and still finish the install.
#[test]
fn a_legacy_install_warns_and_continues_through_the_finalizer() {
    let root = TempDir::new().expect("a temporary root should be created");
    let dirs = mock_dirs(root.path());
    let args = InstallArgs::parse_from(["whitaker-installer"]);
    let snapshot = snapshot_in(root.path());
    // A staging path that does not exist fails the record's canonicalize
    // step, while the bootstrap write, which does not canonicalize, still
    // succeeds. That is the only way to reach the continuation branch: a
    // failure at the record path itself would also fail the bootstrap.
    let staging_path = Utf8PathBuf::from_path_buf(root.path().join("absent"))
        .expect("a temporary path should be UTF-8");
    let context = FinishInstallContext {
        args: &args,
        dirs: &dirs,
        snapshot: &snapshot,
        toolchain: "nightly-2026-05-28",
        staging_path: &staging_path,
        wrapper_scripts: WrapperScripts::Both,
        install_mode: InstallMode::Build,
        install_started: Instant::now(),
    };
    let mut stderr = Vec::new();

    finish_install_and_record_metrics(&context, &mut stderr)
        .expect("the legacy installer warns and continues");

    let output = String::from_utf8(stderr).expect("the output should be UTF-8");
    assert!(
        output.contains("Warning: "),
        "the legacy installer should warn: {output}"
    );
    assert!(
        output.contains("Wrapper scripts created:"),
        "the legacy installer should still finish: {output}"
    );
}
