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
use ortho_config::Localizer;

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
    diagnostics::DiagnosticSettings,
    ports::{CheckPrepared, CheckRepairError, CheckRunError, CheckServices},
};

/// What the user asked `check` to do, and the root flags that colour it.
///
/// The root flags travel with the request so an implicit repair inherits them:
/// `whitaker -q check` must not print an installer's progress.
#[derive(Debug, Clone, Copy)]
pub struct CheckRequest<'a> {
    /// The subcommand's own arguments, including the repair policy.
    pub arguments: &'a CheckArgs,
    /// The verbosity flags the root command held.
    pub settings: DiagnosticSettings,
}

impl<'a> CheckRequest<'a> {
    /// Pair a parsed `check` invocation with the root flags it was parsed under.
    #[must_use]
    pub const fn new(arguments: &'a CheckArgs, settings: DiagnosticSettings) -> Self {
        Self {
            arguments,
            settings,
        }
    }
}

/// Run `whitaker check` and report how it ended.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use ortho_config::NoOpLocalizer;
/// use whitaker::{
///     adapters::SystemServices,
///     cli::{
///         check::{CheckArgs, CheckRequest, run_check_command},
///         exit::Outcome,
///     },
///     diagnostics::DiagnosticSettings,
/// };
///
/// let services = SystemServices::new();
/// // `--no-install` is what makes refusal the expected answer: without it an
/// // unprovisioned machine is repaired and then linted.
/// let arguments = CheckArgs {
///     no_install: true,
///     ..CheckArgs::default()
/// };
/// let request = CheckRequest::new(&arguments, DiagnosticSettings::default());
/// let outcome = run_check_command(request, &NoOpLocalizer::new(), &services, &mut io::sink());
/// // A machine with no installation refuses rather than linting.
/// assert!(
///     matches!(
///         outcome,
///         Outcome::Check(whitaker::cli::check::exit::CheckOutcome::Refused(_))
///     ),
///     "an absent installation must be refused, got {outcome:?}"
/// );
/// ```
pub fn run_check_command(
    request: CheckRequest<'_>,
    localizer: &dyn Localizer,
    services: &dyn CheckServices,
    stderr: &mut dyn Write,
) -> Outcome {
    let policy = policy::policy_for(
        request.arguments.no_install,
        services.library_path_is_user_managed(),
    );
    let mut runner = CheckRunner {
        services,
        settings: request.settings,
        cargo_args: &request.arguments.cargo_args,
        stderr,
    };
    let outcome = telemetry::check_span().in_scope(|| run_check_flow(&mut runner, policy));
    telemetry::record_decision(decision_label(outcome));
    if let CheckOutcome::Refused(reason) = outcome {
        telemetry::record_refusal(reason.label());
    }
    report(outcome, localizer, runner.stderr);
    Outcome::Check(outcome)
}

/// Write the diagnostic a `check` outcome deserves.
///
/// The router calls this when a command failed before `check` ran, such as a
/// help request whose text never reached standard output, so that a `check`
/// failure always names its reason key even when the first diagnostic was
/// lost.
pub(crate) fn report_outcome(
    outcome: CheckOutcome,
    localizer: &dyn Localizer,
    stderr: &mut dyn Write,
) {
    report(outcome, localizer, stderr);
}

/// Write the diagnostic an outcome deserves, without letting it mask the code.
///
/// A lost diagnostic cannot change an outcome that has already been decided,
/// so it is reported through `tracing` instead of being folded into the status
/// the caller sees.
fn report(outcome: CheckOutcome, localizer: &dyn Localizer, stderr: &mut dyn Write) {
    if let Err(error) = messages::report(outcome, localizer, stderr) {
        tracing::error!(%error, "failed to write the check diagnostic to stderr");
    }
}

/// Adapts the `check` port to the pure flow for one invocation.
struct CheckRunner<'a> {
    /// The readiness and run adapter.
    services: &'a dyn CheckServices,
    /// The root command's own diagnostic policy, passed to an implicit install.
    settings: DiagnosticSettings,
    /// The arguments this invocation forwards, before Dylint's own are added.
    cargo_args: &'a [OsString],
    /// Where the child's own diagnostics are written.
    stderr: &'a mut dyn Write,
}

impl CheckPorts for CheckRunner<'_> {
    type Prepared = CheckPrepared;

    /// Translate the port's readiness answer into the flow's vocabulary.
    ///
    /// The adapter hands back either evidence Dylint may run or a typed
    /// failure, and the failure is what the flow classifies: repairable reasons
    /// permit a repair, host faults never do.
    fn preflight(&mut self) -> Preflight<CheckPrepared> {
        let answer = match self.services.preflight() {
            Ok(prepared) => Preflight::Ready(prepared),
            Err(error) => classify_preflight_failure(&error).into_preflight(),
        };
        telemetry::record_preflight(preflight_class(&answer));
        answer
    }

    /// Provision the installation under the adapter's own lock.
    ///
    /// The flow permits this only when [`policy`] allowed it, so the adapter
    /// does not re-check the policy: it re-checks the installation, which is
    /// what makes a concurrent install converge instead of repeat.
    fn repair(&mut self) -> Result<(), RepairFault> {
        let repaired = self.services.repair(self.settings, self.stderr);
        telemetry::record_repair(repaired.is_ok());
        repaired.map_err(|error| {
            // The flow acts on the bounded class, but the cause would otherwise
            // be lost at this boundary: keep it in the log, never in a label.
            tracing::warn!(error = %error, "repair failed; classifying the cause");
            repair_fault(&error)
        })
    }

    /// Run Dylint with the arguments this invocation forwarded.
    ///
    /// The caller's Cargo suffix is assembled here rather than in the flow, and
    /// the preparation the flow selected is passed through unchanged, so the
    /// run uses exactly the installation readiness approved.
    fn run(&mut self, prepared: CheckPrepared) -> Result<i32, RunFault> {
        let arguments = assemble_dylint_args(self.cargo_args);
        self.services
            .check_run(prepared, &arguments, self.stderr)
            .map_err(|error| {
                tracing::warn!(error = %error, "lint run failed; classifying the cause");
                run_fault(error)
            })
    }
}

/// Name the class of one readiness answer for telemetry.
///
/// The names are the readiness vocabulary rather than the error text, so a
/// dashboard can group answers without following a path or a message.
const fn preflight_class(answer: &Preflight<CheckPrepared>) -> &'static str {
    match answer {
        Preflight::Ready(_) => "ready",
        Preflight::Repairable(reason) => reason.label(),
        Preflight::Host(fault) => fault.label(),
    }
}

/// Name the branch the flow took, for telemetry.
const fn decision_label(outcome: CheckOutcome) -> &'static str {
    match outcome {
        CheckOutcome::Linted(_) => "linted",
        CheckOutcome::ConfigurationFailed => "configuration-failed",
        CheckOutcome::Refused(_) => "refused",
        CheckOutcome::RepairFailed => "repair-failed",
        CheckOutcome::NotConverged(_) => "not-converged",
        CheckOutcome::HostFailed(_) => "host-failed",
        CheckOutcome::RunFailed => "run-failed",
        CheckOutcome::OutputFailed => "output-failed",
    }
}

/// Classify a failed repair by whether retrying could ever help.
const fn repair_fault(error: &CheckRepairError) -> RepairFault {
    match error {
        CheckRepairError::Install { .. } => RepairFault::Failed,
        CheckRepairError::Lock { .. } => RepairFault::Host(HostFault::Lock),
        CheckRepairError::DataDirectory => RepairFault::Host(HostFault::DataDirectory),
        CheckRepairError::Output { .. } => RepairFault::Output,
        CheckRepairError::Host { .. } => RepairFault::Host(HostFault::Io),
    }
}

/// Classify a failed run by what, if anything, can fix it.
fn run_fault(error: CheckRunError) -> RunFault {
    match error {
        CheckRunError::Preflight(failure) => provisioning(classify_preflight_failure(&failure)),
        CheckRunError::Bootstrap(failure) => provisioning(classify_failure(&failure)),
        CheckRunError::Spawn { .. } => RunFault::Spawn,
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

#[cfg(test)]
mod tests;
