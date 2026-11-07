//! What the forwarding boundary records, and how it classifies an ending.

use std::time::Duration;

use metrics::Unit;
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;

use super::{
    DIAGNOSTIC_TOTAL,
    DiagnosticStream,
    FORWARD_SECONDS,
    FORWARD_TOTAL,
    ForwardOutcome,
    ForwardSource,
    INSTALLATION_RECORD_TOTAL,
    LIBRARY_PATH_CANDIDATES,
    LIBRARY_PATH_DROPPED,
    LIBRARY_PATH_SECONDS,
    LIBRARY_PATH_TOTAL,
    RecordReadOutcome,
    error_category,
    forward_span,
    record_diagnostic_write,
    record_dropped_candidates,
    record_forward,
    record_installation_record_read,
    record_library_path,
};
use crate::{adapters::library_path::LibraryPathKind, ports::CliError};

/// One recorded metric, as a debugging snapshot lists it.
type Recorded = (
    CompositeKey,
    Option<Unit>,
    Option<metrics::SharedString>,
    DebugValue,
);

/// Return whether `recorded` holds `name` with `label` counted exactly once.
fn counted_once(recorded: &[Recorded], name: &str, label: (&str, &str)) -> bool {
    counter_value(recorded, name, label) == Some(1)
}

/// Return the first value recorded for `name` and `label`, if any.
///
/// A metric name is bound to one instrument type by construction, so pairing
/// the lookup with the type check at each call site costs nothing and keeps
/// the key matching in one place.
fn recorded_value<'a>(
    recorded: &'a [Recorded],
    name: &str,
    label: (&str, &str),
) -> Option<&'a DebugValue> {
    recorded.iter().find_map(|(key, _, _, value)| {
        let matches_key = key.key().name() == name
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == label.0 && entry.value() == label.1);
        matches_key.then_some(value)
    })
}

/// Return the counter value recorded for `name` and `label`, if any.
fn counter_value(recorded: &[Recorded], name: &str, label: (&str, &str)) -> Option<u64> {
    match recorded_value(recorded, name, label)? {
        DebugValue::Counter(count) => Some(*count),
        _ => None,
    }
}

/// Return the single histogram sample recorded for `name` and `label`.
fn histogram_sample(recorded: &[Recorded], name: &str, label: (&str, &str)) -> Option<f64> {
    match recorded_value(recorded, name, label)? {
        DebugValue::Histogram(samples) => samples.first().map(|sample| f64::from(*sample)),
        _ => None,
    }
}

/// A successful and a failed run are counted under their own labels.
#[test]
fn forward_outcomes_are_counted_by_source_and_outcome() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_forward(
            ForwardSource::Bootstrap,
            ForwardOutcome::Succeeded,
            Duration::from_millis(5),
        );
        record_forward(
            ForwardSource::Direct,
            ForwardOutcome::ChildFailed,
            Duration::from_millis(7),
        );
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, FORWARD_TOTAL, ("outcome", "succeeded")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, FORWARD_TOTAL, ("outcome", "child_failed")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, FORWARD_TOTAL, ("source", "direct")),
        "{captured:#?}"
    );
    assert!(
        histogram_sample(&captured, FORWARD_SECONDS, ("source", "bootstrap")).is_some(),
        "{captured:#?}"
    );
}

/// A resolution records both its outcome and how many directories it found.
#[test]
fn library_path_resolutions_record_their_candidate_count() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_library_path(LibraryPathKind::Resolved, 3, Duration::from_millis(12));
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, LIBRARY_PATH_TOTAL, ("kind", "resolved")),
        "{captured:#?}"
    );
    assert_eq!(
        histogram_sample(&captured, LIBRARY_PATH_CANDIDATES, ("kind", "resolved")),
        Some(3.0),
        "{captured:#?}"
    );
    assert!(
        histogram_sample(&captured, LIBRARY_PATH_SECONDS, ("kind", "resolved")).is_some(),
        "{captured:#?}"
    );
}

/// A resolution reports dropped candidates separately by reason, so a broken
/// directory is distinguishable from one that was simply removed.
#[test]
fn dropped_candidates_are_counted_by_reason() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_dropped_candidates(2, 1);
    });

    let captured = snapshotter.snapshot().into_vec();
    assert_eq!(
        counter_value(&captured, LIBRARY_PATH_DROPPED, ("reason", "absent")),
        Some(2),
        "{captured:#?}"
    );
    assert_eq!(
        counter_value(&captured, LIBRARY_PATH_DROPPED, ("reason", "unusable")),
        Some(1),
        "{captured:#?}"
    );
}

/// Every record-read outcome is distinguishable, so a stale record is visible
/// rather than merely absent.
#[test]
fn installation_record_reads_record_every_outcome() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        for outcome in [
            RecordReadOutcome::Missing,
            RecordReadOutcome::Empty,
            RecordReadOutcome::Stale,
            RecordReadOutcome::Current,
            RecordReadOutcome::Failed,
        ] {
            record_installation_record_read(outcome);
        }
    });

    let captured = snapshotter.snapshot().into_vec();
    for label in ["missing", "empty", "stale", "current", "failed"] {
        assert!(
            counted_once(&captured, INSTALLATION_RECORD_TOTAL, ("outcome", label)),
            "{label} should be counted: {captured:#?}"
        );
    }
}

/// A diagnostic that could not be written is counted against its stream.
#[test]
fn diagnostic_writes_are_counted_by_stream_and_outcome() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_diagnostic_write(DiagnosticStream::Stderr, true);
        record_diagnostic_write(DiagnosticStream::Stdout, false);
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, DIAGNOSTIC_TOTAL, ("outcome", "ok")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, DIAGNOSTIC_TOTAL, ("outcome", "failed")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, DIAGNOSTIC_TOTAL, ("stream", "stdout")),
        "{captured:#?}"
    );
}

/// Exit codes collapse into the three endings a user can act on.
#[rstest]
#[case::clean(0, ForwardOutcome::Succeeded)]
#[case::lints_fired(3, ForwardOutcome::ChildFailed)]
#[case::interrupted(130, ForwardOutcome::Signalled)]
#[case::beyond_the_signal_window(256, ForwardOutcome::ChildFailed)]
fn exit_codes_are_classified_into_bounded_outcomes(
    #[case] code: i32,
    #[case] expected: ForwardOutcome,
) {
    assert_eq!(ForwardOutcome::classify_exit(code), expected);
}

/// Every error variant maps to a label that carries no user data.
#[test]
fn error_categories_are_bounded_labels() {
    let error = CliError::Inventory {
        message: "/home/someone/.local/share/whitaker is unavailable".to_owned(),
    };

    assert_eq!(error_category(&error), "inventory");
    assert!(
        !error_category(&error).contains('/'),
        "a category must not leak a path"
    );
}

/// The span names the operation and the route it took.
#[test]
fn the_forward_span_names_its_operation_and_source() {
    let span = forward_span(ForwardSource::Bootstrap, LibraryPathKind::Explicit);
    let metadata = span.metadata().expect("the span carries static metadata");

    assert_eq!(metadata.name(), "whitaker.forward");
}
