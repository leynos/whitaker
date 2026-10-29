//! Bounded operational metrics for snapshot provisioning and production.
//!
//! As in `bootstrap`, the installer installs no recorder, and every label is a
//! fixed constant rather than a path, commit, or error message.

pub(crate) const PROVISION_TOTAL: &str = "whitaker_installer.clippy_snapshot.provision.total";
pub(crate) const CLONE_ATTEMPT_TOTAL: &str =
    "whitaker_installer.clippy_snapshot.clone_attempt.total";

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

/// Record one producer clone attempt: `ok`, `retry`, or `exhausted`.
pub(crate) fn record_clone_attempt(outcome: &'static str) {
    describe_metrics();
    metrics::counter!(CLONE_ATTEMPT_TOTAL, "outcome" => outcome).increment(1);
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
        CLONE_ATTEMPT_TOTAL,
        "Producer clones of upstream Clippy by attempt outcome"
    );
}
