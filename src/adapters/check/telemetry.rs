//! Bounded diagnostics for the `check` command.
//!
//! Labels here are drawn from a closed set — the [`RepairableReason`] labels —
//! so metric cardinality cannot follow a user's filesystem or an error
//! message. The root binary installs no exporter, so these counters are inert
//! unless a host embeds the crate and installs a recorder.
//!
//! [`RepairableReason`]: crate::ports::RepairableReason

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
pub fn record_refusal(reason: Decision) {
    describe_metrics();
    counter!(CHECK_REFUSALS, "reason" => reason.label()).increment(1);
}

/// Report the class of one readiness answer.
///
/// The class is a `&'static str` drawn from the readiness vocabulary, so the
/// event never carries an error message or a path.
pub fn record_preflight(class: Decision) {
    tracing::info!(preflight = class.label(), "readiness classified");
}

/// Report the branch the flow took for one invocation.
///
/// Like [`record_preflight`], the label is static: an operator can group
/// invocations by branch without the label following user input. A completed
/// lint run is routine and is reported at `info`; every other branch means the
/// user did not get a lint result, so it is reported at `warn` and a default
/// subscriber shows it without extra configuration.
pub fn record_decision(decision: Decision) {
    let label = decision.label();
    if label == LINTED {
        report_linted(label);
    } else {
        report_operational(label);
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

/// Run `flow` inside the `whitaker.check` span and record its outcome.
///
/// The command layer supplies the work and the two labels; this module owns
/// which events an invocation emits, so the command layer names no telemetry
/// function and no metric. Recording inside the span is what gives the decision
/// event the invocation's context.
pub fn observe_invocation<T>(
    flow: impl FnOnce() -> T,
    describe: impl FnOnce(&T) -> (Decision, bool),
    refusal: impl FnOnce(&T) -> Option<Decision>,
) -> T {
    check_span().in_scope(|| {
        let outcome = flow();
        let (decision, failed) = describe(&outcome);
        record_decision(decision);
        if let Some(reason) = refusal(&outcome) {
            record_refusal(reason);
        }
        if failed {
            record_outcome(decision);
        }
        outcome
    })
}

/// The bounded decision label one invocation reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// The label, drawn from the flow's closed vocabulary.
    pub label: &'static str,
}

impl Decision {
    /// Name the branch the flow took.
    #[must_use]
    pub const fn new(label: &'static str) -> Self { Self { label } }

    /// The label this decision reports.
    #[must_use]
    pub const fn label(self) -> &'static str { self.label }
}

/// Classify `preflight`, then record which class it answered.
pub fn observe_preflight<T>(
    preflight: impl FnOnce() -> T,
    classify: impl FnOnce(&T) -> Decision,
) -> T {
    let answer = preflight();
    record_preflight(classify(&answer));
    answer
}

/// Time `repair`, then record whether it succeeded and how long it took.
///
/// The clock is a parameter, so a caller that must not read the wall clock —
/// and a test that must not wait for it — supplies its own. Production passes
/// [`MonotonicClock`], the standard library's monotonic source, which is also
/// what the adapter's own retries use.
pub fn observe_repair<T>(
    clock: &dyn Clock,
    repair: impl FnOnce() -> T,
    succeeded: impl FnOnce(&T) -> bool,
) -> T {
    let started = clock.now();
    let outcome = repair();
    record_repair(succeeded(&outcome), clock.elapsed(started));
    outcome
}

/// A source of elapsed time, injected so a caller controls the clock.
///
/// `elapsed` is part of the trait rather than computed from
/// `Instant::elapsed`, so a scripted clock can report a duration without the
/// test waiting for one.
pub trait Clock {
    /// Mark the moment a timed operation starts.
    fn now(&self) -> std::time::Instant;

    /// Report how long has passed since `since`.
    fn elapsed(&self, since: std::time::Instant) -> std::time::Duration;
}

/// The process's monotonic clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    fn now(&self) -> std::time::Instant { std::time::Instant::now() }

    fn elapsed(&self, since: std::time::Instant) -> std::time::Duration { since.elapsed() }
}

/// Count one operational failure under its bounded decision label.
///
/// Called only for outcomes that failed after readiness was settled; refusals
/// are counted by [`record_refusal`] and a lint run is not a failure.
pub fn record_outcome(decision: Decision) {
    describe_metrics();
    counter!(CHECK_OUTCOMES, "decision" => decision.label()).increment(1);
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
#[path = "telemetry_tests.rs"]
mod tests;
