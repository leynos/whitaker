//! Turning port answers and typed failures into the flow's vocabulary.
//!
//! The command layer reports what happened; it does not decide what to do about
//! it. These mappings are the translation between the ports' typed answers and
//! the bounded labels the flow and telemetry consume, kept beside each other so
//! a new variant cannot reach one without being considered by the others.

use super::flow::{RepairFault, RunFault};
use crate::ports::{
    CheckOutcome,
    CheckRepairError,
    CheckRunError,
    Decision,
    HostFault,
    Preflight,
    PreflightFailure,
    classify_failure,
    classify_preflight_failure,
};

/// Name the class of one readiness answer for telemetry.
///
/// The names are the readiness vocabulary rather than the error text, so a
/// dashboard can group answers without following a path or a message.
pub(super) const fn preflight_class<P>(answer: &Preflight<P>) -> &'static str {
    match answer {
        Preflight::Ready(_) => "ready",
        Preflight::Repairable(reason) => reason.label(),
        Preflight::Host(fault) => fault.label(),
    }
}

/// Whether an outcome failed after readiness was settled.
///
/// A refusal is counted by its own reason counter, and a completed lint run is
/// the success case, so neither is an operational failure here.
pub(super) const fn is_operational_failure(outcome: CheckOutcome) -> bool {
    matches!(
        outcome,
        CheckOutcome::ConfigurationFailed
            | CheckOutcome::RepairFailed
            | CheckOutcome::NotConverged(_)
            | CheckOutcome::HostFailed(_)
            | CheckOutcome::RunFailed
            | CheckOutcome::OutputFailed
    )
}

/// Name the branch the flow took, for telemetry.
///
/// The mapping is total, so a new outcome cannot reach telemetry without a
/// decision to report it under.
pub(super) const fn decision(outcome: CheckOutcome) -> Decision {
    match outcome {
        CheckOutcome::Linted(_) => Decision::Linted,
        CheckOutcome::ConfigurationFailed => Decision::ConfigurationFailed,
        CheckOutcome::Refused(_) => Decision::Refused,
        CheckOutcome::RepairFailed => Decision::RepairFailed,
        CheckOutcome::NotConverged(_) => Decision::NotConverged,
        CheckOutcome::HostFailed(_) => Decision::HostFailed,
        CheckOutcome::RunFailed => Decision::RunFailed,
        CheckOutcome::OutputFailed => Decision::OutputFailed,
    }
}

/// Classify a failed repair by whether retrying could ever help.
pub(super) const fn repair_fault(error: &CheckRepairError) -> RepairFault {
    match error {
        CheckRepairError::Install { .. } => RepairFault::Failed,
        CheckRepairError::Lock { .. } => RepairFault::Host(HostFault::Lock),
        CheckRepairError::DataDirectory => RepairFault::Host(HostFault::DataDirectory),
        CheckRepairError::Output { .. } => RepairFault::Output,
        CheckRepairError::Host { .. } => RepairFault::Host(HostFault::Io),
    }
}

/// Classify a failed run by what, if anything, can fix it.
///
/// [`CheckRunError::Unapproved`] means the run could not start at all: the
/// adapter held no approved installation for the value it was handed. That is
/// Whitaker's own wiring rather than a fact about the machine, so it is
/// reported as a run that never started, spends no repair, and offers no
/// install advice. The true cause is already in the warn log the caller writes
/// before classifying.
pub(super) fn run_fault(error: CheckRunError) -> RunFault {
    match error {
        CheckRunError::Preflight(failure) => provisioning(classify_preflight_failure(&failure)),
        CheckRunError::Bootstrap(failure) => provisioning(classify_failure(&failure)),
        CheckRunError::Unapproved | CheckRunError::Spawn { .. } => RunFault::Spawn,
        CheckRunError::Output { .. } => RunFault::Output,
    }
}

/// Turn a classified readiness failure into the verdict a run acts on.
const fn provisioning(failure: PreflightFailure) -> RunFault {
    match failure {
        PreflightFailure::Repairable(reason) => RunFault::Provisioning(reason),
        PreflightFailure::Host(fault) => RunFault::Host(fault),
    }
}
