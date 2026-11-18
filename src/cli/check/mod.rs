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

use std::{ffi::OsString, io::Write};

pub use args::CheckArgs;

use self::{
    argv::assemble_dylint_args,
    classify::{Preflight, PreflightFailure, classify_failure, classify_preflight_failure},
    exit::CheckOutcome,
    flow::{CheckPorts, Policy, RepairFault, RunFault, run_check_flow},
};
use crate::{
    adapters::check::telemetry,
    cli::exit::Outcome,
    ports::{CheckPrepared, CheckRunError, CheckServices},
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
    let mut runner = CheckRunner {
        services,
        cargo_args: &arguments.cargo_args,
        stderr,
    };
    let outcome = telemetry::check_span().in_scope(|| run_check_flow(&mut runner, policy()));
    if let CheckOutcome::Refused(reason) = outcome {
        telemetry::record_refusal(reason.label());
    }
    report(outcome, runner.stderr);
    Outcome::Check(outcome)
}

/// Decide what this invocation may do about a broken installation.
///
/// Lazy repair lands in `EP-M2`, so nothing may provision yet and a repairable
/// defect always becomes a refusal with advice to run `whitaker install`.
const fn policy() -> Policy {
    Policy {
        repair_allowed: false,
    }
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

    /// Provision the installation.
    ///
    /// Repair arrives in `EP-M2`. Until then [`policy`] never permits it, so
    /// this is unreachable; it reports a plain failure rather than pretending
    /// to install anything.
    fn repair(&mut self) -> Result<(), RepairFault> { Err(RepairFault::Failed) }

    fn run(&mut self, prepared: CheckPrepared) -> Result<i32, RunFault> {
        let arguments = assemble_dylint_args(self.cargo_args);
        self.services
            .check_run(prepared, &arguments, self.stderr)
            .map_err(run_fault)
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
mod tests {
    //! Wiring tests: the port is driven, and the flow's rules reach it.

    use std::{
        ffi::OsString,
        io::{self, Write},
        sync::Mutex,
    };

    use super::{CheckArgs, CheckServices, Outcome, run_check_command};
    use crate::ports::{CheckPreflightError, CheckPrepared, CheckRunError};

    /// A port that answers from a script and records what it was asked.
    struct ScriptedPort {
        /// What the readiness check answers.
        answer: Result<(), CheckPreflightError>,
        /// The status a run reports.
        status: i32,
        /// The arguments each run received.
        executed: Mutex<Vec<Vec<OsString>>>,
    }

    impl ScriptedPort {
        fn new(answer: Result<(), CheckPreflightError>) -> Self {
            Self {
                answer,
                status: 0,
                executed: Mutex::new(Vec::new()),
            }
        }
    }

    impl CheckServices for ScriptedPort {
        fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError> {
            self.answer.clone().map(|()| CheckPrepared::Direct {
                library_path: OsString::from("/lints"),
            })
        }

        fn check_run(
            &self,
            _prepared: CheckPrepared,
            args: &[OsString],
            _stderr: &mut dyn Write,
        ) -> Result<i32, CheckRunError> {
            self.executed
                .lock()
                .expect("the test owns this lock")
                .push(args.to_vec());
            Ok(self.status)
        }
    }

    fn arguments(cargo_args: &[&str]) -> CheckArgs {
        CheckArgs {
            no_install: false,
            cargo_args: cargo_args.iter().map(OsString::from).collect(),
        }
    }

    #[test]
    fn a_ready_installation_reaches_the_port_with_whitakers_own_arguments() {
        let port = ScriptedPort::new(Ok(()));

        let outcome = run_check_command(&arguments(&["-p", "x"]), &port, &mut io::sink());

        assert_eq!(
            outcome,
            Outcome::Check(super::CheckOutcome::Linted(0)),
            "a clean run must report Dylint's own status"
        );
        assert_eq!(
            port.executed
                .lock()
                .expect("the test owns this lock")
                .as_slice(),
            [vec![
                OsString::from("--lib=whitaker_suite"),
                OsString::from("--no-metadata"),
                OsString::from("--"),
                OsString::from("-p"),
                OsString::from("x"),
            ]],
            "the port must receive Whitaker's arguments before the user's"
        );
    }

    #[test]
    fn a_missing_installation_is_refused_before_any_run() {
        let port = ScriptedPort::new(Err(CheckPreflightError::Bootstrap(
            whitaker_installer::bootstrap::BootstrapError::NotInstalled {
                path: "bootstrap.json".to_owned(),
            },
        )));
        let mut stderr = Vec::new();

        let outcome = run_check_command(&arguments(&[]), &port, &mut stderr);

        assert_eq!(
            outcome,
            Outcome::Check(super::CheckOutcome::Refused(
                super::classify::RepairableReason::NotInstalled
            )),
            "a repairable defect with repair forbidden must be refused"
        );
        assert!(
            port.executed
                .lock()
                .expect("the test owns this lock")
                .is_empty(),
            "Dylint must not run against an installation that was never approved"
        );
        let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
        assert!(
            rendered.contains("whitaker: error[provision.refused]:"),
            "the refusal must print its stable key, got {rendered:?}"
        );
        assert!(
            rendered.contains("help: run `whitaker install`"),
            "the refusal must advise provisioning, got {rendered:?}"
        );
    }

    #[test]
    fn a_user_managed_path_is_refused_with_its_own_key() {
        let port = ScriptedPort::new(Err(CheckPreflightError::UserPathHasNoSuite));
        let mut stderr = Vec::new();

        let outcome = run_check_command(&arguments(&[]), &port, &mut stderr);

        assert_eq!(
            outcome,
            Outcome::Check(super::CheckOutcome::Refused(
                super::classify::RepairableReason::UserPath
            )),
            "a user-managed path failure keeps its own reason"
        );
        let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
        assert!(
            rendered.contains("whitaker: error[provision.user-path]:"),
            "a user path failure must print its own key, got {rendered:?}"
        );
    }

    #[test]
    fn a_host_fault_is_reported_without_advice() {
        let port = ScriptedPort::new(Err(CheckPreflightError::Bootstrap(
            whitaker_installer::bootstrap::BootstrapError::NoDataDirectory,
        )));
        let mut stderr = Vec::new();

        let outcome = run_check_command(&arguments(&[]), &port, &mut stderr);

        assert_eq!(
            outcome,
            Outcome::Check(super::CheckOutcome::HostFailed(
                super::classify::HostFault::DataDirectory
            )),
            "an undeterminable data directory is a host fault"
        );
        let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
        assert!(
            !rendered.contains("help:"),
            "installing again cannot mend a host fault, so no advice is offered: {rendered:?}"
        );
    }

    #[test]
    fn every_repairable_reason_has_a_stable_label() {
        for reason in [
            super::classify::RepairableReason::NotInstalled,
            super::classify::RepairableReason::Metadata,
            super::classify::RepairableReason::LibrariesMissing,
            super::classify::RepairableReason::Snapshot,
            super::classify::RepairableReason::CargoDylint,
            super::classify::RepairableReason::DylintVersion,
            super::classify::RepairableReason::UserPath,
        ] {
            let label = reason.label();
            assert!(
                !label.is_empty() && label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "a metric label must be a stable identifier, got {label:?}"
            );
        }
    }
}
