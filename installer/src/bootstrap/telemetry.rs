//! Bounded operational metrics for `whitaker-installer run`.
//!
//! The installer installs no recorder, so these are no-ops unless an embedding
//! process provides one. Every label value is a fixed constant: no paths,
//! toolchains, or error text reach a label.

use super::error::BootstrapError;
use std::time::Duration;

pub(crate) const RUN_TOTAL: &str = "whitaker_installer.bootstrap.run.total";
pub(crate) const RUN_SECONDS: &str = "whitaker_installer.bootstrap.run.duration_seconds";
pub(crate) const LOCK_WAIT_SECONDS: &str =
    "whitaker_installer.bootstrap.lock_wait.duration_seconds";
pub(crate) const LOCK_FAILURE_TOTAL: &str = "whitaker_installer.bootstrap.lock_failure.total";
pub(crate) const DRIVER_BUILD_SECONDS: &str =
    "whitaker_installer.bootstrap.driver_build.duration_seconds";
pub(crate) const DRIVER_BUILD_TOTAL: &str = "whitaker_installer.bootstrap.driver_build.total";

/// The outcome label of a run that succeeded, whatever the child's status.
pub(crate) const OUTCOME_OK: &str = "ok";

/// How a run found the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriverPath {
    /// The run failed before it could tell whether the driver was ready.
    Unresolved,
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
            Self::Unresolved => "unresolved",
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
    /// The snapshot failed its full verification, so no build was attempted.
    VerifyFailed,
    /// `cargo dylint` could not be started.
    SpawnFailed,
}

impl BuildOutcome {
    /// The bounded label value for this outcome.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Built => "built",
            Self::NotBuilt => "not_built",
            Self::Unrecorded => "unrecorded",
            Self::VerifyFailed => "verify_failed",
            Self::SpawnFailed => "spawn_failed",
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

/// Map a bootstrap failure to its bounded outcome label.
pub(crate) const fn failure_label(error: &BootstrapError) -> &'static str {
    match error {
        BootstrapError::NoDataDirectory => "no_data_directory",
        BootstrapError::NotInstalled { .. } => "not_installed",
        BootstrapError::Metadata { .. } => "metadata_invalid",
        BootstrapError::LibrariesMissing { .. } => "libraries_missing",
        BootstrapError::Snapshot(_) => "snapshot_invalid",
        BootstrapError::CargoDylint { .. } => "cargo_dylint_unavailable",
        BootstrapError::DylintVersion { .. } => "dylint_version_mismatch",
        BootstrapError::Io { .. } => "io",
        BootstrapError::Spawn { .. } => "spawn_failed",
    }
}

/// Record one finished run: the path it took, its outcome, and its duration.
pub(crate) fn record_run(path: DriverPath, outcome: &'static str, elapsed: Duration) {
    describe_metrics();
    metrics::counter!(RUN_TOTAL, "driver" => path.label(), "outcome" => outcome).increment(1);
    metrics::histogram!(RUN_SECONDS, "driver" => path.label()).record(elapsed.as_secs_f64());
}

/// Record how long a run waited for the driver lock.
///
/// A long exclusive wait is the signature of contended cold construction.
pub(crate) fn record_lock_wait(mode: LockMode, waited: Duration) {
    describe_metrics();
    metrics::histogram!(LOCK_WAIT_SECONDS, "mode" => mode.label()).record(waited.as_secs_f64());
}

/// Record a driver lock that could not be opened or taken.
pub(crate) fn record_lock_failure(mode: LockMode) {
    describe_metrics();
    metrics::counter!(LOCK_FAILURE_TOTAL, "mode" => mode.label()).increment(1);
}

/// Record a cold run's duration and what it left behind.
pub(crate) fn record_driver_build(outcome: BuildOutcome, elapsed: Duration) {
    describe_metrics();
    metrics::histogram!(DRIVER_BUILD_SECONDS, "outcome" => outcome.label())
        .record(elapsed.as_secs_f64());
    metrics::counter!(DRIVER_BUILD_TOTAL, "outcome" => outcome.label()).increment(1);
}

/// Describe the bootstrap metrics to the active recorder.
fn describe_metrics() {
    // Descriptions belong to the active recorder; a process-wide guard could
    // register them only with a no-op or test-local recorder.
    metrics::describe_counter!(
        RUN_TOTAL,
        "Bootstrap runs categorized by driver path and bounded outcome"
    );
    metrics::describe_histogram!(
        RUN_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of bootstrap runs, including the child cargo dylint"
    );
    metrics::describe_histogram!(
        LOCK_WAIT_SECONDS,
        metrics::Unit::Seconds,
        "Time a bootstrap run waited for the driver lock"
    );
    metrics::describe_counter!(
        LOCK_FAILURE_TOTAL,
        "Driver locks that could not be opened or taken"
    );
    metrics::describe_histogram!(
        DRIVER_BUILD_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of cold runs, which verify the snapshot and build the driver"
    );
    metrics::describe_counter!(
        DRIVER_BUILD_TOTAL,
        "Cold bootstrap runs categorized by what they left in the driver slot"
    );
}
