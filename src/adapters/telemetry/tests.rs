//! What the forwarding boundary records, and how it classifies an ending.

use std::time::Duration;

use metrics::Unit;
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;

use super::{
    FORWARD_SECONDS,
    FORWARD_TOTAL,
    ForwardOutcome,
    ForwardSource,
    INSTALLATION_RECORD_TOTAL,
    LIBRARY_PATH_CANDIDATES,
    LIBRARY_PATH_TOTAL,
    error_category,
    forward_span,
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
    recorded.iter().any(|(key, _, _, value)| {
        key.key().name() == name
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == label.0 && entry.value() == label.1)
            && matches!(value, DebugValue::Counter(1))
    })
}

/// Return the single histogram sample recorded for `name` and `label`.
fn histogram_sample(recorded: &[Recorded], name: &str, label: (&str, &str)) -> Option<f64> {
    recorded.iter().find_map(|(key, _, _, value)| {
        let matches_key = key.key().name() == name
            && key
                .key()
                .labels()
                .any(|entry| entry.key() == label.0 && entry.value() == label.1);
        match value {
            DebugValue::Histogram(samples) if matches_key => {
                samples.first().map(|sample| f64::from(*sample))
            }
            _ => None,
        }
    })
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
        record_library_path(LibraryPathKind::Resolved, 3);
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
}

/// Both record-read outcomes are distinguishable, so a broken record is
/// visible rather than merely absent.
#[test]
fn installation_record_reads_record_both_outcomes() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_installation_record_read(true);
        record_installation_record_read(false);
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        counted_once(&captured, INSTALLATION_RECORD_TOTAL, ("outcome", "ok")),
        "{captured:#?}"
    );
    assert!(
        counted_once(&captured, INSTALLATION_RECORD_TOTAL, ("outcome", "failed")),
        "{captured:#?}"
    );
}

/// Exit codes collapse into the three endings a user can act on.
#[rstest]
#[case::clean(0, ForwardOutcome::Succeeded)]
#[case::lints_fired(3, ForwardOutcome::ChildFailed)]
#[case::interrupted(130, ForwardOutcome::Signalled)]
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
