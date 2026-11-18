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
///     Some("whitaker.check")
/// );
/// ```
#[must_use]
pub fn check_span() -> Span { tracing::info_span!("whitaker.check", operation = "check") }

/// Record one refusal under its bounded reason label.
pub fn record_refusal(reason: &'static str) {
    describe_metrics();
    counter!(CHECK_REFUSALS, "reason" => reason).increment(1);
}

#[cfg(test)]
mod tests {
    //! What the `check` boundary records, through the collection path.

    use metrics::Unit;
    use metrics_util::{
        CompositeKey,
        debugging::{DebugValue, DebuggingRecorder},
    };

    use super::{CHECK_REFUSALS, record_refusal};

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
