//! The `whitaker check` command: readiness, the decision flow, and its output.
//!
//! The command has three parts and they are separated on purpose. This module
//! is the adapter between the [`CheckServices`] port and the pure flow; the
//! flow itself decides what to do and cannot perform I/O; and `messages`
//! turns the result into the two lines a person reads. Keeping the decision
//! free of ports is what lets the Kani harnesses drive every branch.

mod args;
pub mod argv;
pub mod classify;
pub mod exit;
pub mod flow;
mod messages;
pub mod policy;

use std::{ffi::OsString, io::Write};

pub use args::CheckArgs;

use self::{
    argv::assemble_dylint_args,
    classify::{
        HostFault,
        Preflight,
        PreflightFailure,
        classify_failure,
        classify_preflight_failure,
    },
    exit::CheckOutcome,
    flow::{CheckPorts, RepairFault, RunFault, run_check_flow},
};
use crate::{
    adapters::check::telemetry,
    cli::exit::Outcome,
    ports::{CheckPrepared, CheckRepairError, CheckRunError, CheckServices},
};

/// Run `whitaker check` and report how it ended.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use whitaker::{
///     adapters::SystemServices,
///     cli::{
///         check::{CheckArgs, run_check_command},
///         exit::Outcome,
///     },
/// };
///
/// let services = SystemServices::new();
/// let outcome = run_check_command(&CheckArgs::default(), &services, &mut io::sink());
/// // A machine with no installation refuses rather than linting.
/// assert!(matches!(outcome, Outcome::Check(_)));
/// ```
pub fn run_check_command(
    arguments: &CheckArgs,
    services: &dyn CheckServices,
    stderr: &mut dyn Write,
) -> Outcome {
    let policy = policy::policy_for(
        arguments.no_install,
        services.library_path_is_user_managed(),
    );
    let mut runner = CheckRunner {
        services,
        cargo_args: &arguments.cargo_args,
        stderr,
    };
    let outcome = telemetry::check_span().in_scope(|| run_check_flow(&mut runner, policy));
    if let CheckOutcome::Refused(reason) = outcome {
        telemetry::record_refusal(reason.label());
    }
    report(outcome, runner.stderr);
    Outcome::Check(outcome)
}

/// Write the diagnostic a `check` outcome deserves.
///
/// The router calls this when a command failed before `check` ran, such as a
/// help request whose text never reached standard output, so that a `check`
/// failure always names its reason key even when the first diagnostic was
/// lost.
pub(crate) fn report_outcome(outcome: CheckOutcome, stderr: &mut dyn Write) {
    report(outcome, stderr);
}

/// Write the diagnostic an outcome deserves, without letting it mask the code.
///
/// A lost diagnostic cannot change an outcome that has already been decided,
/// so it is reported through `tracing` instead of being folded into the status
/// the caller sees.
fn report(outcome: CheckOutcome, stderr: &mut dyn Write) {
    if let Err(error) = messages::report(outcome, stderr) {
        tracing::error!(%error, "failed to write the check diagnostic to stderr");
    }
}

/// Adapts the `check` port to the pure flow for one invocation.
struct CheckRunner<'a> {
    /// The readiness and run adapter.
    services: &'a dyn CheckServices,
    /// The arguments this invocation forwards, before Dylint's own are added.
    cargo_args: &'a [OsString],
    /// Where the child's own diagnostics are written.
    stderr: &'a mut dyn Write,
}

impl CheckPorts for CheckRunner<'_> {
    type Prepared = CheckPrepared;

    fn preflight(&mut self) -> Preflight<CheckPrepared> {
        match self.services.preflight() {
            Ok(prepared) => Preflight::Ready(prepared),
            Err(error) => classify_preflight_failure(&error).into_preflight(),
        }
    }

    /// Provision the installation under the adapter's own lock.
    ///
    /// The flow permits this only when [`policy`] allowed it, so the adapter
    /// does not re-check the policy: it re-checks the installation, which is
    /// what makes a concurrent install converge instead of repeat.
    fn repair(&mut self) -> Result<(), RepairFault> {
        let repaired = self.services.repair(self.stderr);
        telemetry::record_repair(repaired.is_ok());
        repaired.map_err(|error| repair_fault(&error))
    }

    fn run(&mut self, prepared: CheckPrepared) -> Result<i32, RunFault> {
        let arguments = assemble_dylint_args(self.cargo_args);
        self.services
            .check_run(prepared, &arguments, self.stderr)
            .map_err(run_fault)
    }
}

/// Classify a failed repair by whether retrying could ever help.
const fn repair_fault(error: &CheckRepairError) -> RepairFault {
    match error {
        CheckRepairError::Install { .. } => RepairFault::Failed,
        CheckRepairError::Lock { .. } => RepairFault::Host(HostFault::Lock),
        CheckRepairError::DataDirectory => RepairFault::Host(HostFault::DataDirectory),
        CheckRepairError::Host { .. } => RepairFault::Host(HostFault::Io),
    }
}

/// Classify a failed run by what, if anything, can fix it.
fn run_fault(error: CheckRunError) -> RunFault {
    match error {
        CheckRunError::Bootstrap(failure) => match classify_failure(&failure) {
            PreflightFailure::Repairable(reason) => RunFault::Provisioning(reason),
            PreflightFailure::Host(fault) => RunFault::Host(fault),
        },
        CheckRunError::Spawn { .. } => RunFault::Spawn,
        CheckRunError::Output { .. } => RunFault::Output,
    }
}

#[cfg(test)]
mod tests;
