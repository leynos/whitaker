//! Snapshot operation and clone-attempt metrics carry bounded outcomes.

use super::*;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use rstest::rstest;

/// Return the `outcome` label of the one operation counter `run` recorded.
fn recorded_outcome(run: impl FnOnce()) -> String {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, run);
    let recorded = snapshotter.snapshot().into_vec();
    let counters: Vec<_> = recorded
        .iter()
        .filter(|(key, _, _, value)| {
            key.key().name() == OPERATION_TOTAL && matches!(value, DebugValue::Counter(1))
        })
        .collect();
    assert_eq!(counters.len(), 1, "{recorded:#?}");
    let has_duration = recorded.iter().any(|(key, _, _, value)| {
        key.key().name() == OPERATION_SECONDS
            && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
    });
    assert!(has_duration, "{recorded:#?}");
    counters[0]
        .0
        .key()
        .labels()
        .find(|label| label.key() == "outcome")
        .map(|label| label.value().to_owned())
        .expect("an outcome label")
}

#[test]
fn a_successful_operation_records_ok() {
    let outcome = recorded_outcome(|| {
        timed(Operation::Install, || Ok(())).expect("succeeds");
    });

    assert_eq!(outcome, OUTCOME_OK);
}

#[rstest]
#[case::io(ClippySnapshotError::Io { path: "/x".into(), reason: "denied".into() }, "io")]
#[case::not_installed(ClippySnapshotError::NotInstalled { path: "/x".into() }, "not_installed")]
fn a_failed_operation_records_its_bounded_label(
    #[case] error: ClippySnapshotError,
    #[case] expected: &str,
) {
    let outcome = recorded_outcome(|| {
        timed(Operation::Download, || Err::<(), _>(error)).expect_err("fails");
    });

    assert_eq!(outcome, expected);
}

#[rstest]
#[case::first_of_three(1, 3, "retry")]
#[case::second_of_three(2, 3, "retry")]
#[case::last_of_three(3, 3, "exhausted")]
fn a_failed_clone_attempt_is_a_retry_until_attempts_run_out(
    #[case] attempt: u32,
    #[case] attempts: u32,
    #[case] expected: &str,
) {
    assert_eq!(failed_attempt_label(attempt, attempts), expected);
}
