//! The query/command split of `run`, its error-aware probes, and its metrics.
//!
//! These run against a real installed snapshot and a stub `cargo`, so they
//! are Unix-only; `installer/tests/bootstrap_run.rs` covers the binary.

use super::*;
use crate::bootstrap::telemetry::{
    DRIVER_BUILD_SECONDS, DRIVER_BUILD_TOTAL, LOCK_WAIT_SECONDS, RUN_TOTAL,
};
use crate::bootstrap::{BOOTSTRAP_SCHEMA, StoredPath};
use crate::test_utils::{FIXTURE_TOOLCHAIN, SnapshotFixture, fixture_git};
use metrics_util::CompositeKey;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use rstest::rstest;
use tempfile::TempDir;

/// A stub `cargo` that reports Dylint 6.3.0 and, when `builds` is set,
/// writes a driver into the exported slot as Dylint would.
fn stub_cargo(builds: bool) -> String {
    format!(
        r#"#!/usr/bin/env bash
if [[ "$1" == dylint && "$2" == --version ]]; then
    echo "cargo-dylint 6.3.0"
    exit 0
fi
if [[ {builds} == yes ]]; then
    mkdir -p "$DYLINT_DRIVER_PATH/{FIXTURE_TOOLCHAIN}-x86_64-unknown-linux-gnu"
    printf 'driver' >"$DYLINT_DRIVER_PATH/{FIXTURE_TOOLCHAIN}-x86_64-unknown-linux-gnu/dylint-driver"
fi
"#,
        builds = if builds { "yes" } else { "no" },
    )
}

/// A complete installation in a temporary data root.
struct Installed {
    _dir: TempDir,
    _fixture: SnapshotFixture,
    layout: BootstrapLayout,
    library_dir: Utf8PathBuf,
    commit: String,
}

impl Installed {
    /// Install a snapshot, a lint directory, and the bootstrap metadata.
    fn new() -> Self {
        let dir = TempDir::new().expect("create temp dir");
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8 temp path");
        let data_root = base.join("data");
        let layout = BootstrapLayout::at(&data_root.join("whitaker-bootstrap"));
        let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
        fixture
            .install(&layout.clippy_root())
            .expect("install the fixture snapshot");
        let commit = fixture.commit().to_owned();
        let library_dir = data_root.join("whitaker/lints/lib");
        std::fs::create_dir_all(&library_dir).expect("create library dir");
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: commit.clone(),
            toolchain: FIXTURE_TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_root),
        }
        .write(&layout.metadata_path())
        .expect("write metadata");
        Self {
            _dir: dir,
            _fixture: fixture,
            layout,
            library_dir,
            commit,
        }
    }

    /// Write a stub `cargo` beside the data root and return its path.
    fn cargo(&self, builds: bool) -> Utf8PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let name = if builds { "cargo-builds" } else { "cargo-idle" };
        let path = self.library_dir.join("../../..").join(name);
        std::fs::write(&path, stub_cargo(builds)).expect("write stub cargo");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod stub cargo");
        path
    }

    /// The driver slot this installation resolves to.
    fn slot_dir(&self) -> Utf8PathBuf {
        self.layout.driver_dir("6.3.0", &self.commit)
    }
}

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
    let installed = Installed::new();
    let cargo = installed.cargo(true);

    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");

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
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    let parent = installed
        .library_dir
        .parent()
        .expect("library dir has a parent");
    std::fs::remove_dir_all(parent).expect("remove the lint directory");
    // A file where a directory should be: inspecting the child fails with
    // `NotADirectory`, which is not absence.
    std::fs::write(parent, b"not a directory").expect("write a file in its place");

    let error = prepare(&installed.layout, cargo.as_os_str()).expect_err("cannot inspect");

    assert!(matches!(error, BootstrapError::Io { .. }), "{error:?}");
}

#[test]
fn a_missing_library_dir_is_still_reported_as_missing() {
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    std::fs::remove_dir_all(&installed.library_dir).expect("remove the library dir");

    let error = prepare(&installed.layout, cargo.as_os_str()).expect_err("missing");

    assert!(
        matches!(error, BootstrapError::LibrariesMissing { .. }),
        "{error:?}"
    );
}

#[rstest]
#[case::builds_a_driver(true, "built")]
#[case::leaves_the_slot_cold(false, "not_built")]
fn a_cold_run_records_its_path_and_outcome(#[case] builds: bool, #[case] outcome: &str) {
    let installed = Installed::new();
    let cargo = installed.cargo(builds);
    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let code =
        metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
            .expect("execute");

    assert_eq!(code, 0);
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
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
    execute(&prepared, cargo.as_os_str(), &[]).expect("cold run");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
        .expect("warm run");

    let recorded = snapshotter.snapshot().into_vec();
    assert!(counted_once(&recorded, RUN_TOTAL, ("driver", "warm")));
    assert!(
        !recorded
            .iter()
            .any(|(key, _, _, _)| key.key().name() == DRIVER_BUILD_TOTAL),
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
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
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
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
    prepared.slot.create().expect("create the slot");
    let holder = prepared
        .slot
        .lock_exclusive()
        .expect("hold the exclusive lock");

    let worker = {
        let prepared = prepared.clone();
        let cargo = cargo.clone();
        std::thread::spawn(move || {
            let recorder = DebuggingRecorder::new();
            let snapshotter = recorder.snapshotter();
            metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
                .expect("the run completes once the lock is released");
            snapshotter.snapshot().into_vec()
        })
    };
    std::thread::sleep(HELD);
    drop(holder);
    let recorded = worker.join().expect("join the worker");

    let exclusive_wait = recorded
        .iter()
        .filter(|(key, _, _, _)| {
            key.key().name() == LOCK_WAIT_SECONDS
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "mode" && l.value() == "exclusive")
        })
        .filter_map(|(_, _, _, value)| match value {
            DebugValue::Histogram(samples) => samples.first().map(|s| s.into_inner()),
            _ => None,
        })
        .next()
        .expect("an exclusive lock wait was recorded");
    // The hold is 400 ms; a worker that started within 350 ms of it waited at
    // least 50 ms. A wait near zero would mean the lock was never contended.
    assert!(exclusive_wait >= 0.05, "waited {exclusive_wait}s");
    assert!(
        counted_once(&recorded, RUN_TOTAL, ("driver", "cold")),
        "{recorded:#?}"
    );
}
