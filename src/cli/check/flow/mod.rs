//! The readiness, repair, and run flow behind `whitaker check`.
//!
//! The flow is deliberately free of adapters: [`CheckPorts`] is narrow enough
//! that a test — and an exhaustive `Kani` harness — can drive every choice it
//! has. That matters because three of its rules fail silently when wrong:
//! repair must never run against a ready installation or a host fault, it must
//! run at most once, and Dylint must never run against an installation the
//! flow has not just seen ready.

use super::{
    classify::{HostFault, Preflight, RepairableReason},
    exit::CheckOutcome,
};

/// The installation-facing operations the flow needs.
///
/// `Prepared` is whatever value proves that a readiness check passed. The flow
/// never inspects it: it only hands the value from a `Ready` preflight to
/// [`Self::run`], which is what makes "Dylint runs only against a fresh,
/// approved installation" structural rather than a rule to remember.
pub trait CheckPorts {
    /// The proof-of-readiness value a successful preflight returns.
    type Prepared;

    /// Report whether Dylint can run, without running it.
    fn preflight(&mut self) -> Preflight<Self::Prepared>;

    /// Provision the installation, then return once it should be ready.
    ///
    /// # Errors
    ///
    /// Returns [`RepairFault`] when provisioning failed, when Whitaker could
    /// not write its own output, or when the host failed before it could
    /// start.
    fn repair(&mut self) -> Result<(), RepairFault>;

    /// Run Dylint against a prepared installation and return its status.
    ///
    /// # Errors
    ///
    /// Returns [`RunFault`] when the installation stopped being usable, when
    /// the child could not be started, or when Whitaker could not write its
    /// own output.
    fn run(&mut self, prepared: Self::Prepared) -> Result<i32, RunFault>;
}

/// Why provisioning did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairFault {
    /// The installation ran and failed.
    Failed,
    /// Whitaker could not write its own output before installation started.
    Output,
    /// The host failed before installation could start, such as a lock that
    /// could not be taken. Retrying would fail the same way.
    Host(HostFault),
}

/// Why Dylint could not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunFault {
    /// The installation stopped being ready between the preflight and the run.
    Provisioning(RepairableReason),
    /// `cargo dylint` could not be started.
    Spawn,
    /// The host failed while Dylint was being started or run.
    Host(HostFault),
    /// Whitaker could not write its own output.
    Output,
}

/// Whether this invocation may provision Whitaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Whether a repairable installation may be repaired in this invocation.
    pub repair_allowed: bool,
}

/// The action the flow takes for one readiness result.
#[derive(Debug)]
pub enum Step<P> {
    /// Run Dylint against this validated installation.
    Run(P),
    /// Provision the installation and check again.
    Repair(RepairableReason),
    /// Report that provisioning is needed but not permitted.
    Refuse(RepairableReason),
    /// Report a host fault.
    Fail(HostFault),
}

/// Decide what to do about one readiness result.
///
/// Taking the preflight by value is what keeps a [`Step::Run`] honest: the
/// only `P` in existence came from a `Ready` answer, so no caller can run
/// Dylint against an installation it has not just validated.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::{
///     classify::Preflight,
///     flow::{Policy, Step, decide},
/// };
///
/// let allowed = Policy {
///     repair_allowed: true,
/// };
/// let refused = Policy {
///     repair_allowed: false,
/// };
///
/// assert!(
///     matches!(decide(Preflight::Ready(7), refused), Step::Run(7)),
///     "a ready installation must run even when repair is forbidden"
/// );
/// assert!(
///     matches!(
///         decide::<i32>(
///             Preflight::Repairable(
///                 whitaker::cli::check::classify::RepairableReason::NotInstalled
///             ),
///             allowed
///         ),
///         Step::Repair(_)
///     ),
///     "a repairable installation must be repaired when repair is allowed"
/// );
/// ```
#[must_use]
pub fn decide<P>(preflight: Preflight<P>, policy: Policy) -> Step<P> {
    match preflight {
        Preflight::Ready(prepared) => Step::Run(prepared),
        Preflight::Repairable(reason) if policy.repair_allowed => Step::Repair(reason),
        Preflight::Repairable(reason) => Step::Refuse(reason),
        Preflight::Host(fault) => Step::Fail(fault),
    }
}

/// Run the whole flow and report one outcome.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::flow::{Policy, run_check_flow};
/// # use whitaker::cli::check::classify::Preflight;
/// # use whitaker::cli::check::flow::{CheckPorts, RepairFault, RunFault};
/// # struct Port;
/// # impl CheckPorts for Port {
/// #     type Prepared = ();
/// #     fn preflight(&mut self) -> Preflight<()> { Preflight::Ready(()) }
/// #     fn repair(&mut self) -> Result<(), RepairFault> { Ok(()) }
/// #     fn run(&mut self, (): ()) -> Result<i32, RunFault> { Ok(0) }
/// # }
/// let outcome = run_check_flow(
///     &mut Port,
///     Policy {
///         repair_allowed: false,
///     },
/// );
/// assert_eq!(
///     outcome,
///     whitaker::cli::check::exit::CheckOutcome::Linted(0),
///     "a ready installation must lint"
/// );
/// ```
#[must_use]
pub fn run_check_flow<P: CheckPorts>(ports: &mut P, policy: Policy) -> CheckOutcome {
    match decide(ports.preflight(), policy) {
        Step::Run(prepared) => ran(ports, prepared, policy),
        Step::Refuse(reason) => CheckOutcome::Refused(reason),
        Step::Fail(fault) => CheckOutcome::HostFailed(fault),
        Step::Repair(_) => repair_then_lint(ports),
    }
}

/// Lint, and spend the one repair if the run finds the installation unusable.
///
/// Readiness is a snapshot of the filesystem, and the run is the first moment
/// it can be contradicted: a suite removed after the preflight, or a snapshot
/// whose full verification fails on the cold path, surfaces as a typed
/// provisioning failure from the run itself. Treating that as final would
/// leave the user re-running a command that fails identically every time.
fn ran<P: CheckPorts>(ports: &mut P, prepared: P::Prepared, policy: Policy) -> CheckOutcome {
    match ports.run(prepared) {
        Err(RunFault::Provisioning(_)) if policy.repair_allowed => repair_then_lint(ports),
        result => classified(result),
    }
}

/// Provision once, then lint against whatever the second check approves.
fn repair_then_lint<P: CheckPorts>(ports: &mut P) -> CheckOutcome {
    if let Err(fault) = ports.repair() {
        return repair_failed(fault);
    }
    // Repair is spent. Asking again with it forbidden keeps a failed install
    // from looping: the second decision can only run, refuse, or fail.
    let forbidden = Policy {
        repair_allowed: false,
    };
    match decide(ports.preflight(), forbidden) {
        Step::Run(prepared) => classified(ports.run(prepared)),
        Step::Refuse(reason) | Step::Repair(reason) => CheckOutcome::NotConverged(reason),
        Step::Fail(fault) => CheckOutcome::HostFailed(fault),
    }
}

/// Turn a run's report into the outcome the command exits with.
///
/// A user-managed path that stopped naming one suite is refused, not reported
/// as a repair that did not converge: the path belongs to the user, so no
/// repair could have changed it, and the refusal names the key and advice that
/// route's readiness failure already uses.
const fn classified(result: Result<i32, RunFault>) -> CheckOutcome {
    match result {
        Ok(code) => CheckOutcome::Linted(code),
        Err(RunFault::Provisioning(RepairableReason::UserPath)) => {
            CheckOutcome::Refused(RepairableReason::UserPath)
        }
        Err(RunFault::Provisioning(reason)) => CheckOutcome::NotConverged(reason),
        Err(RunFault::Spawn) => CheckOutcome::RunFailed,
        Err(RunFault::Host(fault)) => CheckOutcome::HostFailed(fault),
        Err(RunFault::Output) => CheckOutcome::OutputFailed,
    }
}

/// Classify a failed provisioning attempt.
const fn repair_failed(fault: RepairFault) -> CheckOutcome {
    match fault {
        RepairFault::Failed => CheckOutcome::RepairFailed,
        RepairFault::Output => CheckOutcome::OutputFailed,
        RepairFault::Host(host) => CheckOutcome::HostFailed(host),
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
pub(crate) mod samplers;

#[cfg(kani)]
mod script;

#[cfg(kani)]
mod verification;

#[cfg(kani)]
mod witnesses;
