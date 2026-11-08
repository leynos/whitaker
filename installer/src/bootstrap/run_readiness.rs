//! Ready-marker bookkeeping for the bootstrap's driver slot.
//!
//! Every helper here decides what a finished child run means for the shared
//! driver slot and reports that decision. They are separated from the run
//! sequencing in [`super::run`] so that module stays within the repository's
//! file-size limit, and so the "mark ready, or report why not" policy reads in
//! one place.
//!
//! None of these helpers fail the run: the child has already executed, and its
//! exit status matters more than the marker. A slot that cannot be marked is
//! simply cold again next time.

use super::{
    driver::DriverSlot,
    error::BootstrapError,
    telemetry::{BuildOutcome, failure_label},
};

/// Record a rebuilt driver's verification outcome in the slot.
///
/// A verified replacement is marked ready so other runs may share it; a failed
/// verification clears the marker so the next cold run rebuilds from scratch.
pub(super) fn record_verification(
    slot: &DriverSlot,
    verified: &Result<(), BootstrapError>,
) -> BuildOutcome {
    if verified.is_ok() {
        mark_rebuilt_ready(slot)
    } else {
        clear_rebuilt_ready(slot)
    }
}

/// Mark a verified rebuild ready, reporting but not returning a marker failure.
fn mark_rebuilt_ready(slot: &DriverSlot) -> BuildOutcome {
    if let Err(error) = slot.mark_ready() {
        log_ready_mark_failure(&error);
    }
    BuildOutcome::Built
}

/// Clear the ready marker after a failed verification.
fn clear_rebuilt_ready(slot: &DriverSlot) -> BuildOutcome {
    if let Err(error) = slot.clear_ready() {
        log_ready_clear_failure(&error);
    }
    BuildOutcome::VerifyFailed
}

/// Report a ready marker that could not be written.
fn log_ready_mark_failure(error: &BootstrapError) {
    tracing::warn!(
        failure = failure_label(error),
        "could not record the rebuilt driver as ready"
    );
}

/// Report a ready marker that could not be cleared.
fn log_ready_clear_failure(error: &BootstrapError) {
    tracing::warn!(
        failure = failure_label(error),
        "could not mark the driver cold after a failed verification"
    );
}

/// Mark the slot ready when the child left a driver behind.
///
/// Failures here are reported but not returned: the child has already run,
/// and its exit status matters more than the marker. An unmarked slot is
/// simply cold again next time.
pub(super) fn record_readiness(slot: &DriverSlot) -> BuildOutcome {
    match slot.has_driver() {
        Ok(true) => mark_ready_outcome(slot),
        Ok(false) => report_missing_driver(slot),
        Err(error) => report_unreadable_driver(&error),
    }
}

/// Report a child that exited without leaving a driver behind.
fn report_missing_driver(slot: &DriverSlot) -> BuildOutcome {
    tracing::warn!(
        toolchain = slot.toolchain(),
        "cargo dylint exited without building a driver"
    );
    BuildOutcome::NotBuilt
}

/// Report a driver slot that could not be inspected after the child ran.
fn report_unreadable_driver(error: &BootstrapError) -> BuildOutcome {
    tracing::warn!(
        failure = failure_label(error),
        "could not record the driver as ready"
    );
    BuildOutcome::Unrecorded
}

/// Mark the slot ready after the child left a driver behind.
fn mark_ready_outcome(slot: &DriverSlot) -> BuildOutcome {
    match slot.mark_ready() {
        Ok(()) => BuildOutcome::Built,
        Err(error) => {
            tracing::warn!(
                failure = failure_label(&error),
                "could not record the driver as ready"
            );
            BuildOutcome::Unrecorded
        }
    }
}
