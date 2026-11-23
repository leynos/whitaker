//! Bounded diagnostics for the `check` command.
//!
//! Labels here are drawn from a closed set — the [`RepairableReason`] labels —
//! so metric cardinality cannot follow a user's filesystem or an error
//! message. The root binary installs no exporter, so these counters are inert
//! unless a host embeds the crate and installs a recorder.
//!
//! [`RepairableReason`]: crate::cli::check::classify::RepairableReason

use std::time::Duration;

use metrics::{counter, describe_counter};
use tracing::Span;

/// Counts invocations refused because the installation needed provisioning.
pub const CHECK_REFUSALS: &str = "whitaker.check.refusals";

/// Counts operational failures, by the flow's bounded decision label.
///
/// Only outcomes that failed after readiness was settled are counted here:
/// `configuration-failed`, `repair-failed`, `not-converged`, `host-failed`,
/// `run-failed` and `output-failed`. A refusal has its own counter
/// ([`CHECK_REFUSALS`]), and a completed lint run is not a failure at all, so
/// neither appears here. The label is drawn from a closed set, never from an
/// error message or a path.
pub const CHECK_OUTCOMES: &str = "whitaker.check.outcomes";

/// Counts repair requests, by whether the install call itself succeeded.
///
/// A request counts even when the installation lock was already converged by a
/// concurrent check: the caller asked for provisioning, and the answer it got
/// was success. `ok` therefore means the installer returned success, not that
/// the fresh preflight converged; convergence is the flow's `not-converged`
/// decision label. Dashboards must read this as demand for repair, not as the
/// number of installations performed.
pub const CHECK_REPAIRS: &str = "whitaker.check.repairs";

/// Describe every metric this module emits to the active recorder.
///
/// Descriptions belong to the active recorder, so they are registered on each
/// record call rather than through a process-wide guard that could bind them
/// to a no-op or test-local recorder.
fn describe_metrics() {
    describe_counter!(
        CHECK_REFUSALS,
        "Check invocations refused because the installation needed provisioning, by reason"
    );
    describe_counter!(
        CHECK_REPAIRS,
        "Check invocations that asked for a repair, by whether the install call succeeded; a \
         request may complete without installing when another check provisioned first, and \
         convergence is reported by the not-converged decision label"
    );
    describe_counter!(
        CHECK_OUTCOMES,
        "Check invocations that failed after readiness was settled, by the flow's bounded \
         decision label; refusals are counted separately and a lint run is not a failure"
    );
}

/// Open the span that covers one `check` invocation.
///
/// # Examples
///
/// ```
/// use whitaker::adapters::check::telemetry::check_span;
///
/// assert_eq!(
///     check_span().metadata().map(|meta| meta.name()),
///     Some("whitaker.check"),
///     "the check span must be named for the operation it covers"
/// );
/// ```
#[must_use]
pub fn check_span() -> Span {
    tracing::info_span!(
        "whitaker.check",
        operation = "check",
        repair_ms = tracing::field::Empty
    )
}

/// Record one refusal under its bounded reason label.
pub fn record_refusal(reason: &'static str) {
    describe_metrics();
    counter!(CHECK_REFUSALS, "reason" => reason).increment(1);
}

/// Report the class of one readiness answer.
///
/// The class is a `&'static str` drawn from the readiness vocabulary, so the
/// event never carries an error message or a path.
pub fn record_preflight(class: &'static str) {
    tracing::info!(preflight = class, "readiness classified");
}

/// Report the branch the flow took for one invocation.
///
/// Like [`record_preflight`], the label is static: an operator can group
/// invocations by branch without the label following user input. A completed
/// lint run is routine and is reported at `info`; every other branch means the
/// user did not get a lint result, so it is reported at `warn` and a default
/// subscriber shows it without extra configuration.
pub fn record_decision(decision: &'static str) {
    if decision == LINTED {
        report_linted(decision);
    } else {
        report_operational(decision);
    }
}

/// The one decision label that means the user received a lint result.
const LINTED: &str = "linted";

/// Report a completed lint run at `info`.
fn report_linted(decision: &'static str) {
    tracing::info!(decision, "check decision");
}

/// Report a branch that produced no lint result at `warn`.
fn report_operational(decision: &'static str) {
    tracing::warn!(decision, "check decision");
}

/// Count one operational failure under its bounded decision label.
///
/// Called only for outcomes that failed after readiness was settled; refusals
/// are counted by [`record_refusal`] and a lint run is not a failure.
pub fn record_outcome(decision: &'static str) {
    describe_metrics();
    counter!(CHECK_OUTCOMES, "decision" => decision).increment(1);
}

/// Record one repair attempt under its bounded outcome label.
///
/// The label is `ok` or `failed`, so it never carries an error message or a
/// path: the counter answers "how often was a repair asked for, and did the
/// install succeed?" It does not answer whether linting could then start.
/// `elapsed` is recorded on the event and on the enclosing `whitaker.check`
/// span; it is deliberately not a metric label, which would make the counter
/// unbounded.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use whitaker::adapters::check::telemetry::record_repair;
///
/// record_repair(true, Duration::from_millis(12));
/// ```
pub fn record_repair(succeeded: bool, elapsed: Duration) {
    describe_metrics();
    let outcome = if succeeded { "ok" } else { "failed" };
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    // The span is disabled outside `check_span().in_scope(...)`, where recording
    // a field is a no-op, so this is safe for a direct caller.
    Span::current().record("repair_ms", elapsed_ms);
    tracing::info!(outcome, elapsed_ms, "repair completed");
    counter!(CHECK_REPAIRS, "outcome" => outcome).increment(1);
}

#[cfg(test)]
mod tests {
    //! What the `check` boundary records, through the collection path.

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
            record_preflight("not_installed");
            record_decision("refused");
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
            record_decision("linted");
            record_decision("host-failed");
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
        record_refusal as fn(&'static str),
        ExpectedCounter { name: CHECK_REFUSALS, label: "reason", value: "not_installed" }
    )]
    #[case::operational_failure(
        record_outcome as fn(&'static str),
        ExpectedCounter { name: CHECK_OUTCOMES, label: "decision", value: "not-converged" }
    )]
    fn a_counter_reaches_the_recorder_under_its_label(
        #[case] record: fn(&'static str),
        #[case] expected: ExpectedCounter,
    ) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || record(expected.value));

        let captured = snapshotter.snapshot().into_vec();
        assert!(
            counted_once(&captured, expected),
            "{} must produce a non-zero counter: {captured:#?}",
            expected.name
        );
    }
}
