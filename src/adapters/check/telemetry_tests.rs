//! What the `check` boundary records, through the collection path.
//!
//! These live beside the module they cover so the recorder functions and
//! their bounded labels are pinned next to the code that emits them, while
//! the module itself stays within the length budget the lint allows.

use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use metrics::Unit;
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use tracing::subscriber::with_default;
use tracing_subscriber::fmt::MakeWriter;

use super::{
    CHECK_OUTCOMES,
    CHECK_REFUSALS,
    Decision,
    record_decision,
    record_outcome,
    record_preflight,
    record_refusal,
    record_repair,
};

/// A writer that keeps what a subscriber formatted, for a test to read.
#[derive(Clone, Default)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    /// Return everything written so far as text.
    fn written(&self) -> String {
        let bytes = self.0.lock().expect("the buffer is not poisoned").clone();
        String::from_utf8(bytes).expect("the formatter writes UTF-8")
    }
}

impl io::Write for SharedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("the buffer is poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl<'writer> MakeWriter<'writer> for SharedBuffer {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer { self.clone() }
}

/// Assert the three events reach a subscriber with their bounded labels.
#[test]
fn the_events_reach_a_subscriber_with_their_bounded_labels() {
    let buffer = SharedBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(buffer.clone())
        .finish();

    with_default(subscriber, || {
        record_preflight(Decision::new("not_installed"));
        record_decision(Decision::new("refused"));
        record_repair(false, Duration::from_millis(5));
    });

    let written = buffer.written();
    for expected in [
        "preflight=\"not_installed\"",
        "decision=\"refused\"",
        "outcome=\"failed\"",
        "elapsed_ms=5",
    ] {
        assert!(
            written.contains(expected),
            "the subscriber must receive `{expected}`, got: {written}"
        );
    }
}

/// Report a lint run at `info` and an operational failure at `warn`.
///
/// A default subscriber shows `warn` and above, so an operator sees every
/// invocation that produced no lint result without configuring anything,
/// while a routine run stays at the debug/info noise level.
#[test]
fn operational_outcomes_are_reported_at_warn_and_linted_at_info() {
    let buffer = SharedBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(buffer.clone())
        .finish();

    with_default(subscriber, || {
        record_decision(Decision::new("linted"));
        record_decision(Decision::new("host-failed"));
    });

    let written = buffer.written();
    let linted = written
        .lines()
        .find(|line| line.contains("decision=\"linted\""))
        .unwrap_or_default();
    let failed = written
        .lines()
        .find(|line| line.contains("decision=\"host-failed\""))
        .unwrap_or_default();
    assert!(
        linted.contains(" INFO "),
        "a lint run must stay at info: {linted}"
    );
    assert!(
        failed.contains(" WARN "),
        "an operational failure must be reported at warn: {failed}"
    );
}

/// What one recorded counter is expected to contain.
#[derive(Clone, Copy)]
struct ExpectedCounter {
    /// The metric name.
    name: &'static str,
    /// The label key the counter must carry.
    label: &'static str,
    /// The label value the counter must carry.
    value: &'static str,
}

/// Return whether `expected` was counted exactly once in `recorded`.
fn counted_once(
    recorded: &[(
        CompositeKey,
        Option<Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )],
    expected: ExpectedCounter,
) -> bool {
    recorded.iter().any(|(key, _, _, recorded_value)| {
        let matches_key = key.key().name() == expected.name
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == expected.label && entry.value() == expected.value);
        matches_key && matches!(recorded_value, DebugValue::Counter(1))
    })
}

/// Assert each recorder function counts once under its bounded label.
#[rstest::rstest]
#[case::refusal(
    record_refusal as fn(Decision),
    ExpectedCounter { name: CHECK_REFUSALS, label: "reason", value: "not_installed" }
)]
#[case::operational_failure(
    record_outcome as fn(Decision),
    ExpectedCounter { name: CHECK_OUTCOMES, label: "decision", value: "not-converged" }
)]
fn a_counter_reaches_the_recorder_under_its_label(
    #[case] record: fn(Decision),
    #[case] expected: ExpectedCounter,
) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || record(Decision::new(expected.value)));

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, expected),
        "{} must produce a non-zero counter: {captured:#?}",
        expected.name
    );
}
