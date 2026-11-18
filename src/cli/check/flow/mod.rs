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
    /// Returns [`RepairFault`] when provisioning failed, or when the host
    /// failed before it could start.
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
/// assert!(matches!(decide(Preflight::Ready(7), refused), Step::Run(7)));
/// assert!(matches!(
///     decide::<i32>(
///         Preflight::Repairable(whitaker::cli::check::classify::RepairableReason::NotInstalled),
///         allowed
///     ),
///     Step::Repair(_)
/// ));
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
/// assert_eq!(outcome, whitaker::cli::check::exit::CheckOutcome::Linted(0));
/// ```
#[must_use]
pub fn run_check_flow<P: CheckPorts>(ports: &mut P, policy: Policy) -> CheckOutcome {
    match decide(ports.preflight(), policy) {
        Step::Run(prepared) => lint(ports, prepared),
        Step::Refuse(reason) => CheckOutcome::Refused(reason),
        Step::Fail(fault) => CheckOutcome::HostFailed(fault),
        Step::Repair(_) => repair_then_lint(ports),
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
        Step::Run(prepared) => lint(ports, prepared),
        Step::Refuse(reason) | Step::Repair(reason) => CheckOutcome::NotConverged(reason),
        Step::Fail(fault) => CheckOutcome::HostFailed(fault),
    }
}

/// Run Dylint against an approved installation and classify the result.
fn lint<P: CheckPorts>(ports: &mut P, prepared: P::Prepared) -> CheckOutcome {
    match ports.run(prepared) {
        Ok(code) => CheckOutcome::Linted(code),
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
        RepairFault::Host(host) => CheckOutcome::HostFailed(host),
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod verification;
