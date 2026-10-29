//! Bounded operational metrics for `whitaker-installer run`.
//!
//! The installer installs no recorder, so these are no-ops unless an embedding
//! process provides one. Every label value is a fixed constant: no paths,
//! toolchains, or error text reach a label.

use std::time::Duration;

pub(crate) const RUN_TOTAL: &str = "whitaker_installer.bootstrap.run.total";
pub(crate) const LOCK_WAIT_SECONDS: &str =
    "whitaker_installer.bootstrap.lock_wait.duration_seconds";
pub(crate) const DRIVER_BUILD_SECONDS: &str =
    "whitaker_installer.bootstrap.driver_build.duration_seconds";
pub(crate) const DRIVER_BUILD_TOTAL: &str = "whitaker_installer.bootstrap.driver_build.total";

/// How a run found the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriverPath {
    /// Ready on the first check; ran under the shared lock.
    Warm,
    /// Built by another process while this one waited for the exclusive lock.
    WarmAfterWait,
    /// Built by this run under the exclusive lock.
    Cold,
}

impl DriverPath {
    /// The bounded label value for this path.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::WarmAfterWait => "warm_after_wait",
            Self::Cold => "cold",
        }
    }
}

/// What a cold run left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildOutcome {
    /// Dylint wrote a driver and it was recorded as ready.
    Built,
    /// The child exited without writing a driver, so the slot stays cold.
    NotBuilt,
    /// The slot could not be inspected or the ready marker not written.
    Unrecorded,
}

impl BuildOutcome {
    /// The bounded label value for this outcome.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Built => "built",
            Self::NotBuilt => "not_built",
            Self::Unrecorded => "unrecorded",
        }
    }
}

/// The lock mode a run waited for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LockMode {
    /// Shared by warm runs.
    Shared,
    /// Held alone by a run that may build the driver.
    Exclusive,
}

impl LockMode {
    /// The bounded label value for this mode.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Exclusive => "exclusive",
        }
    }
}

/// Record which path a run took to the driver.
pub(crate) fn record_run(path: DriverPath) {
    describe_metrics();
    metrics::counter!(RUN_TOTAL, "driver" => path.label()).increment(1);
}

/// Record how long a run waited for the driver lock.
///
/// A long exclusive wait is the signature of contended cold construction.
pub(crate) fn record_lock_wait(mode: LockMode, waited: Duration) {
    describe_metrics();
    metrics::histogram!(LOCK_WAIT_SECONDS, "mode" => mode.label()).record(waited.as_secs_f64());
}

/// Record a cold run's duration and what it left behind.
pub(crate) fn record_driver_build(outcome: BuildOutcome, elapsed: Duration) {
    describe_metrics();
    metrics::histogram!(DRIVER_BUILD_SECONDS).record(elapsed.as_secs_f64());
    metrics::counter!(DRIVER_BUILD_TOTAL, "outcome" => outcome.label()).increment(1);
}

/// Describe the bootstrap metrics to the active recorder.
fn describe_metrics() {
    // Descriptions belong to the active recorder; a process-wide guard could
    // register them only with a no-op or test-local recorder.
    metrics::describe_counter!(
        RUN_TOTAL,
        "Bootstrap runs categorized by how the driver was found"
    );
    metrics::describe_histogram!(
        LOCK_WAIT_SECONDS,
        metrics::Unit::Seconds,
        "Time a bootstrap run waited for the driver lock"
    );
    metrics::describe_histogram!(
        DRIVER_BUILD_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of cold runs, which build the Dylint driver"
    );
    metrics::describe_counter!(
        DRIVER_BUILD_TOTAL,
        "Cold bootstrap runs categorized by what they left in the driver slot"
    );
}
