//! Bounded operational metrics for snapshot provisioning and production.
//!
//! As in `bootstrap`, the installer installs no recorder, and every label is a
//! fixed constant rather than a path, commit, or error message.

use super::error::ClippySnapshotError;
use std::time::{Duration, Instant};

pub(crate) const PROVISION_TOTAL: &str = "whitaker_installer.clippy_snapshot.provision.total";
pub(crate) const OPERATION_TOTAL: &str = "whitaker_installer.clippy_snapshot.operation.total";
pub(crate) const OPERATION_SECONDS: &str =
    "whitaker_installer.clippy_snapshot.operation.duration_seconds";
pub(crate) const CLONE_ATTEMPT_TOTAL: &str =
    "whitaker_installer.clippy_snapshot.clone_attempt.total";
pub(crate) const CLONE_ATTEMPT_SECONDS: &str =
    "whitaker_installer.clippy_snapshot.clone_attempt.duration_seconds";

/// The outcome label of an operation that succeeded.
pub(crate) const OUTCOME_OK: &str = "ok";

/// A timed snapshot operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    /// Verify and reuse an installed generation.
    Reuse,
    /// Download the release bundle and its sidecar.
    Download,
    /// Clone a bundle into a new generation and verify it.
    Install,
    /// Fully verify a generation.
    Verify,
}

impl Operation {
    /// The bounded label value for this operation.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Reuse => "reuse",
            Self::Download => "download",
            Self::Install => "install",
            Self::Verify => "verify",
        }
    }
}

/// Map a snapshot failure to its bounded outcome label.
pub(crate) const fn failure_label(error: &ClippySnapshotError) -> &'static str {
    match error {
        ClippySnapshotError::Manifest { .. } => "manifest_invalid",
        ClippySnapshotError::BundleMissing { .. } => "bundle_missing",
        ClippySnapshotError::Sidecar { .. } => "sidecar_invalid",
        ClippySnapshotError::ChecksumMismatch { .. } => "checksum_mismatch",
        ClippySnapshotError::AssetNotFound { .. } => "asset_not_found",
        ClippySnapshotError::Download { .. } => "download_failed",
        ClippySnapshotError::NotInstalled { .. } => "not_installed",
        ClippySnapshotError::Invalid { .. } => "invalid",
        ClippySnapshotError::InsufficientHistory { .. } => "insufficient_history",
        ClippySnapshotError::Git { .. } => "git_failed",
        ClippySnapshotError::Io { .. } => "io",
    }
}

/// Run `operation`, recording its outcome and duration and logging both.
///
/// The error reaches the caller unchanged; the log carries only its bounded
/// label, never a path, URL, or message.
pub(crate) fn timed<T>(
    operation: Operation,
    run: impl FnOnce() -> Result<T, ClippySnapshotError>,
) -> Result<T, ClippySnapshotError> {
    let started = Instant::now();
    let result = run();
    let elapsed = started.elapsed();
    let outcome = result.as_ref().map_or_else(failure_label, |_| OUTCOME_OK);
    record_operation(operation, outcome, elapsed);
    let elapsed_ms = millis(elapsed);
    if result.is_ok() {
        tracing::debug!(
            operation = operation.label(),
            elapsed_ms,
            "snapshot operation finished"
        );
    } else {
        tracing::warn!(
            operation = operation.label(),
            outcome,
            elapsed_ms,
            "snapshot operation failed"
        );
    }
    result
}

/// Whole milliseconds in `elapsed`, saturating, for log fields.
pub(crate) fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// Record one operation's outcome and duration.
fn record_operation(operation: Operation, outcome: &'static str, elapsed: Duration) {
    describe_metrics();
    let labels = [("operation", operation.label()), ("outcome", outcome)];
    metrics::counter!(OPERATION_TOTAL, &labels).increment(1);
    metrics::histogram!(OPERATION_SECONDS, &labels).record(elapsed.as_secs_f64());
}

/// Where a provisioned generation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProvisionSource {
    /// An installed generation that verified and was reused.
    Existing,
    /// The bundle published on the Whitaker release.
    Release,
    /// A bundle supplied on disk.
    Local,
}

impl ProvisionSource {
    /// The bounded label value for this source.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Release => "release",
            Self::Local => "local",
        }
    }
}

/// Record one provisioning attempt and whether it succeeded.
pub(crate) fn record_provision(source: ProvisionSource, succeeded: bool) {
    describe_metrics();
    let outcome = if succeeded { "ok" } else { "failed" };
    metrics::counter!(PROVISION_TOTAL, "source" => source.label(), "outcome" => outcome)
        .increment(1);
}

/// Label a failed clone attempt: `retry` while attempts remain, else
/// `exhausted`.
pub(crate) const fn failed_attempt_label(attempt: u32, attempts: u32) -> &'static str {
    if attempt < attempts {
        "retry"
    } else {
        "exhausted"
    }
}

/// Record one producer clone attempt (`ok`, `retry`, or `exhausted`) and how
/// long it took.
pub(crate) fn record_clone_attempt(outcome: &'static str, elapsed: Duration) {
    describe_metrics();
    metrics::counter!(CLONE_ATTEMPT_TOTAL, "outcome" => outcome).increment(1);
    metrics::histogram!(CLONE_ATTEMPT_SECONDS, "outcome" => outcome).record(elapsed.as_secs_f64());
}

/// Describe the snapshot metrics to the active recorder.
fn describe_metrics() {
    // Descriptions belong to the active recorder; a process-wide guard could
    // register them only with a no-op or test-local recorder.
    metrics::describe_counter!(
        PROVISION_TOTAL,
        "Clippy snapshot provisioning attempts by bundle source and outcome"
    );
    metrics::describe_counter!(
        OPERATION_TOTAL,
        "Clippy snapshot operations by operation and bounded outcome"
    );
    metrics::describe_histogram!(
        OPERATION_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of Clippy snapshot operations"
    );
    metrics::describe_counter!(
        CLONE_ATTEMPT_TOTAL,
        "Producer clones of upstream Clippy by attempt outcome"
    );
    metrics::describe_histogram!(
        CLONE_ATTEMPT_SECONDS,
        metrics::Unit::Seconds,
        "Elapsed time of producer clone attempts"
    );
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
