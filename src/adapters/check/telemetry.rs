//! Bounded diagnostics for the `check` command.
//!
//! Labels here are drawn from a closed set — the [`RepairableReason`] labels —
//! so metric cardinality cannot follow a user's filesystem or an error
//! message. The root binary installs no exporter, so these counters are inert
//! unless a host embeds the crate and installs a recorder.
//!
//! [`RepairableReason`]: crate::cli::check::classify::RepairableReason

use metrics::{counter, describe_counter};
use tracing::Span;

/// Counts invocations refused because the installation needed provisioning.
pub const CHECK_REFUSALS: &str = "whitaker.check.refusals";

/// Counts repair requests, by whether they completed.
///
/// A request counts even when the installation lock was already converged by a
/// concurrent check: the caller asked for provisioning, and the answer it got
/// was success. Dashboards must read this as demand for repair, not as the
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
        "Check invocations that asked for a repair, by outcome; a request may complete without \
         installing when another check provisioned first"
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
pub fn check_span() -> Span { tracing::info_span!("whitaker.check", operation = "check") }

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
/// invocations by branch without the label following user input.
pub fn record_decision(decision: &'static str) {
    tracing::info!(decision, "check decision");
}

/// Record one repair attempt under its bounded outcome label.
///
/// The label is `ok` or `failed`, so it never carries an error message or a
/// path: the counter answers "how often did repair run, and did it work?"
///
/// # Examples
///
/// ```
/// use whitaker::adapters::check::telemetry::record_repair;
///
/// record_repair(true);
/// ```
pub fn record_repair(succeeded: bool) {
    describe_metrics();
    let outcome = if succeeded { "ok" } else { "failed" };
    tracing::info!(outcome, "repair completed");
    counter!(CHECK_REPAIRS, "outcome" => outcome).increment(1);
}

#[cfg(test)]
mod tests {
    //! What the `check` boundary records, through the collection path.

    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use metrics::Unit;
    use metrics_util::{
        CompositeKey,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use tracing::subscriber::with_default;
    use tracing_subscriber::fmt::MakeWriter;

    use super::{CHECK_REFUSALS, record_decision, record_preflight, record_refusal, record_repair};

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
            record_repair(false);
        });

        let written = buffer.written();
        for expected in [
            "preflight=\"not_installed\"",
            "decision=\"refused\"",
            "outcome=\"failed\"",
        ] {
            assert!(
                written.contains(expected),
                "the subscriber must receive `{expected}`, got: {written}"
            );
        }
    }

    /// Return whether `name` was counted with `reason` exactly once.
    fn counted_once(
        recorded: &[(
            CompositeKey,
            Option<Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        )],
        name: &str,
        reason: &str,
    ) -> bool {
        recorded.iter().any(|(key, _, _, value)| {
            let matches_key = key.key().name() == name
                && key
                    .key()
                    .labels()
                    .any(|entry| entry.key() == "reason" && entry.value() == reason);
            matches_key && matches!(value, DebugValue::Counter(1))
        })
    }

    /// Assert one refusal is collected under its reason label.
    #[test]
    fn a_refusal_reaches_the_recorder_under_its_reason() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            record_refusal("not_installed");
        });

        let captured = snapshotter.snapshot().into_vec();
        assert!(
            counted_once(&captured, CHECK_REFUSALS, "not_installed"),
            "a refusal must produce a non-zero counter: {captured:#?}"
        );
    }
}
