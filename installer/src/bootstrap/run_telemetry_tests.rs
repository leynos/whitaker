//! How a run classifies its child and which spans it opens.

use super::*;
use crate::bootstrap::telemetry::RUN_TOTAL;
use std::sync::{Arc, Mutex};
use tracing::span::{Attributes, Id};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Records the name of every span opened while it is the subscriber.
#[derive(Clone, Default)]
struct SpanNames(Arc<Mutex<Vec<&'static str>>>);

impl<S: tracing::Subscriber> Layer<S> for SpanNames {
    /// Remember each new span's name.
    fn on_new_span(&self, attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
        self.0
            .lock()
            .expect("span list lock")
            .push(attrs.metadata().name());
    }
}

/// Write `body` as an executable stub `cargo` beside the installation.
fn stub(installed: &Installed, name: &str, body: &str) -> Utf8PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = installed.library_dir.join("../../..").join(name);
    std::fs::write(&path, body).expect("write stub cargo");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod stub cargo");
    path
}

/// A run whose child fails, as when lints fire, is told apart from both a
/// clean run and a bootstrap error.
#[rstest]
#[case::clean(0, "ok")]
#[case::lints_fired(3, "child_nonzero")]
fn a_run_classifies_its_child_exit(#[case] code: i32, #[case] outcome: &str) {
    let installed = Installed::new();
    let cargo = stub(
        &installed,
        "cargo-exits",
        &format!(
            "#!/usr/bin/env bash\n[[ \"$1 $2\" == \"dylint --version\" ]] && {{ echo 'cargo-dylint 6.3.0'; exit 0; }}\nexit {code}\n"
        ),
    );
    let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let finished =
        metrics::with_local_recorder(&recorder, || execute(&prepared, cargo.as_os_str(), &[]))
            .expect("the bootstrap itself succeeds");

    assert_eq!(finished.code, code, "the child's status is kept");
    let recorded = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&recorded, RUN_TOTAL, ("outcome", outcome)),
        "{recorded:#?}"
    );
}

/// Span capture relies on nextest's process per test, which `make test`
/// uses. Under plain `cargo test` the `tracing_config` tests install a global
/// subscriber in the same process, and its `WARN` filter can recache these
/// `INFO` callsites as uninteresting mid-run.
#[test]
fn a_cold_run_opens_spans_for_each_boundary() {
    let installed = Installed::new();
    let cargo = installed.cargo(true);
    let names = SpanNames::default();
    let subscriber = tracing_subscriber::registry().with(names.clone());

    tracing::subscriber::with_default(subscriber, || {
        // Tests running alongside without a subscriber may already have cached
        // "not interested" for these callsites; recompute with this one active.
        tracing::callsite::rebuild_interest_cache();
        let prepared = prepare(&installed.layout, cargo.as_os_str()).expect("prepare");
        execute(&prepared, cargo.as_os_str(), &[]).expect("execute");
    });

    let names = names.0.lock().expect("span list lock").clone();
    for expected in [
        "bootstrap.cargo_dylint_version",
        "clippy_snapshot.verify",
        "clippy_snapshot.git",
        "bootstrap.child",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
}
