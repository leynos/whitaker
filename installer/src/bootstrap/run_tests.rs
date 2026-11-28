//! The query/command split of `run`, its error-aware probes, and its metrics.
//!
//! These run against a real installed snapshot and a stub `cargo`, so they
//! are Unix-only; `installer/tests/bootstrap_run.rs` covers the binary.

use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;

use super::{fixture::Installed, *};
use crate::{
    bootstrap::{
        driver_run::execute,
        telemetry::{DRIVER_BUILD_SECONDS, DRIVER_BUILD_TOTAL, LOCK_WAIT_SECONDS, RUN_TOTAL},
    },
    test_utils::fixture_git,
};

/// One recorded metric, as a debugging snapshot lists it.
type Recorded = (
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
);

/// Return whether `recorded` holds `name` with `label` incremented once.
fn counted_once(recorded: &[Recorded], name: &str, label: (&str, &str)) -> bool {
    recorded.iter().any(|(key, _, _, value)| {
        key.key().name() == name
            && key
                .key()
                .labels()
                .any(|l| l.key() == label.0 && l.value() == label.1)
            && matches!(value, DebugValue::Counter(1))
    })
}

#[test]
fn prepare_resolves_the_run_without_creating_the_slot() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");

    let prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");

    assert_eq!(
        prepared.snapshot,
        installed.layout.snapshot(&installed.commit)
    );
    assert_eq!(prepared.slot.dir(), installed.slot_dir());
    assert!(
        !installed.slot_dir().exists(),
        "preparing a run is a query and must not create the driver slot"
    );
}

/// Regression: `is_dir` once reported any I/O failure as missing libraries.
#[test]
fn prepare_reports_a_library_dir_it_cannot_inspect() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let parent = installed
        .library_dir
        .parent()
        .expect("library dir has a parent");
    std::fs::remove_dir_all(parent).expect("remove the lint directory");
    // A file where a directory should be: inspecting the child fails with
    // `NotADirectory`, which is not absence.
    std::fs::write(parent, b"not a directory").expect("write a file in its place");

    let error = prepare(&installed.layout, cargo.as_os_str(), RunOptions::default())
        .expect_err("cannot inspect");

    assert!(matches!(error, BootstrapError::Io { .. }), "{error:?}");
}

#[test]
fn a_missing_library_dir_is_still_reported_as_missing() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    std::fs::remove_dir_all(&installed.library_dir).expect("remove the library dir");

    let error =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect_err("missing");

    assert!(
        matches!(error, BootstrapError::LibrariesMissing { .. }),
        "{error:?}"
    );
}

/// Regression: the root CLI forwards its own per-toolchain staging record,
/// and the bootstrap must export that list rather than the installation's.
#[test]
fn an_explicit_library_path_replaces_the_recorded_one() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let override_path = OsStr::new("/custom/lints");
    let options = RunOptions {
        library_path: Some(override_path),
        ..RunOptions::default()
    };

    let prepared = prepare(&installed.layout, cargo.as_os_str(), options).expect("prepare");

    assert_eq!(prepared.library_dir.as_os_str(), override_path);
    // The rest of the installation still supplies the run: only the library
    // path is the caller's.
    assert_eq!(
        prepared.snapshot,
        installed.layout.snapshot(&installed.commit)
    );
    assert_eq!(prepared.slot.dir(), installed.slot_dir());
}

/// An explicit path is authoritative, so a stale recorded directory is not a
/// second, stricter check the caller never asked for.
#[test]
fn an_explicit_library_path_skips_the_recorded_directory_check() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    std::fs::remove_dir_all(&installed.library_dir).expect("remove the library dir");
    let options = RunOptions {
        library_path: Some(OsStr::new("/custom/lints")),
        ..RunOptions::default()
    };

    let prepared = prepare(&installed.layout, cargo.as_os_str(), options)
        .expect("an override needs no recorded directory");

    assert_eq!(
        prepared.library_dir.as_os_str(),
        OsStr::new("/custom/lints")
    );
}

#[rstest]
#[case::builds_a_driver(true, "built")]
#[case::leaves_the_slot_cold(false, "not_built")]
fn a_cold_run_records_its_path_and_outcome(#[case] builds: bool, #[case] outcome: &str) {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(builds).expect("write stub cargo");
    let prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let finished =
        metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
            .expect("execute");

    assert_eq!(finished.code, 0);
    assert_eq!(
        finished.built_driver, builds,
        "only a run that built the driver says so"
    );
    let recorded = snapshotter.snapshot().into_vec();
    assert!(counted_once(&recorded, RUN_TOTAL, ("driver", "cold")));
    assert!(counted_once(
        &recorded,
        DRIVER_BUILD_TOTAL,
        ("outcome", outcome)
    ));
    assert!(recorded.iter().any(|(key, _, _, value)| {
        key.key().name() == DRIVER_BUILD_SECONDS
            && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
    }));
    assert_eq!(prepared.slot.is_ready().expect("inspect the slot"), builds);
}

#[test]
fn a_run_after_the_build_is_warm() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");
    execute(&prepared, cargo.as_os_str(), &[]).expect("cold run");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let finished =
        metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
            .expect("warm run");

    assert!(!finished.built_driver, "a warm run reuses the driver");

    let snapshot_metrics = snapshotter.snapshot().into_vec();
    assert!(counted_once(
        &snapshot_metrics,
        RUN_TOTAL,
        ("driver", "warm")
    ));
    assert!(
        !snapshot_metrics
            .iter()
            .any(|(key, ..)| key.key().name() == DRIVER_BUILD_TOTAL),
        "a warm run builds nothing"
    );
}

/// How a cold run is made to fail before Dylint can build anything.
#[derive(Clone, Copy)]
enum ColdFailure {
    /// The snapshot gains a `master` branch, which full verification refuses.
    SnapshotTampered,
    /// The `cargo` handed to the run does not exist.
    CargoMissing,
}

#[rstest]
#[case::snapshot_tampered(ColdFailure::SnapshotTampered, "verify_failed", "snapshot_invalid")]
#[case::cargo_missing(ColdFailure::CargoMissing, "spawn_failed", "spawn_failed")]
fn a_failed_cold_run_records_why(
    #[case] failure: ColdFailure,
    #[case] build_outcome: &str,
    #[case] run_outcome: &str,
) {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");
    let run_cargo = match failure {
        ColdFailure::SnapshotTampered => {
            fixture_git(&prepared.snapshot, &["branch", "master", "main"])
                .expect("add a master branch");
            cargo
        }
        ColdFailure::CargoMissing => installed.library_dir.join("no-such-cargo"),
    };
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || execute(&prepared, run_cargo.as_os_str(), &[]))
        .expect_err("the cold run fails");

    let recorded = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&recorded, DRIVER_BUILD_TOTAL, ("outcome", build_outcome)),
        "{recorded:#?}"
    );
    assert!(
        counted_once(&recorded, RUN_TOTAL, ("outcome", run_outcome)),
        "{recorded:#?}"
    );
    assert!(!prepared.slot.is_ready().expect("inspect the slot"));
}

/// A run that finds another holding the exclusive lock waits for it, and the
/// wait is what `lock_wait` measures.
#[test]
fn a_contended_cold_run_records_its_lock_wait() {
    const HELD: std::time::Duration = std::time::Duration::from_millis(400);
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        prepare(&installed.layout, cargo.as_os_str(), RunOptions::default()).expect("prepare");
    prepared.slot.create().expect("create the slot");
    let holder = prepared
        .slot
        .lock_exclusive()
        .expect("hold the exclusive lock");

    let worker = {
        let worker_prepared = prepared.clone();
        let worker_cargo = cargo.clone();
        std::thread::spawn(move || {
            let recorder = DebuggingRecorder::new();
            let snapshotter = recorder.snapshotter();
            metrics::with_local_recorder(&recorder, || {
                execute(&worker_prepared, worker_cargo.as_os_str(), &[])
            })
            .expect("the run completes once the lock is released");
            snapshotter.snapshot().into_vec()
        })
    };
    std::thread::sleep(HELD);
    drop(holder);
    let recorded = worker.join().expect("join the worker");

    let exclusive_wait = recorded
        .iter()
        .filter(|(key, ..)| {
            key.key().name() == LOCK_WAIT_SECONDS
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "mode" && l.value() == "exclusive")
        })
        .find_map(|(_, _, _, value)| match value {
            DebugValue::Histogram(samples) => samples.first().map(|s| s.into_inner()),
            _ => None,
        })
        .expect("an exclusive lock wait was recorded");
    // The hold is 400 ms; a worker that started within 350 ms of it waited at
    // least 50 ms. A wait near zero would mean the lock was never contended.
    assert!(exclusive_wait >= 0.05, "waited {exclusive_wait}s");
    assert!(
        counted_once(&recorded, RUN_TOTAL, ("driver", "cold")),
        "{recorded:#?}"
    );
}

#[path = "run_path_tests.rs"]
mod path;

#[path = "run_telemetry_tests.rs"]
mod telemetry;
