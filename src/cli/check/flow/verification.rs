//! Exhaustive checks of the real flow over a finite set of port answers.
//!
//! One limit: [`ProbePort::new`] samples the repairable reason and the host
//! fault once, so every branch of a run sees the same pair. Distinct
//! propagation is covered by the tables in `cli/check/tests/faults.rs`.

use super::{
    CheckPorts,
    Policy,
    RepairFault,
    RunFault,
    run_check_flow,
    samplers::{any_fault, any_reason},
    script::{Answer, RepairAnswer, RunAnswer, Script, script},
};
use crate::{
    cli::check::exit::CheckOutcome,
    ports::{HostFault, Preflight, RepairableReason},
};

/// Sample a policy that may or may not permit provisioning.
fn any_policy() -> Policy {
    Policy {
        repair_allowed: kani::any(),
    }
}

/// The most recent readiness result, as the port recorded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastPreflight {
    /// No readiness check has run yet.
    Unchecked,
    /// A ready installation, carrying the index that proved it.
    Ready(u8),
    /// An installation that needs provisioning.
    Repairable,
    /// A host fault that provisioning cannot fix.
    Host,
}

/// A port whose every answer is a symbolic choice.
///
/// It counts its calls and remembers which preflight a run consumed, so the
/// invariants are stated over calls rather than outcomes.
struct ProbePort {
    /// The answers the flow will be given.
    plan: Script,
    /// How many readiness checks have been answered.
    preflights: u8,
    /// How many provisioning attempts were requested.
    repairs: u8,
    /// How many lint runs were requested.
    runs: u8,
    /// One more than the preflight index the last run consumed, or zero.
    prepared_from: u8,
    /// How many readiness checks had run when the last run started.
    ///
    /// Relative to the run, not the flow: a run-time provisioning failure can
    /// leave a later check that finds the installation still unready.
    preflights_at_last_run: u8,
    /// How many runs consumed something other than the latest ready preflight.
    stale_runs: u8,
    /// The result of the most recent readiness check.
    last_preflight: LastPreflight,
    /// The status a successful run reports.
    code: i32,
    /// The repairable reason reported.
    reason: RepairableReason,
    /// The host fault reported.
    fault: HostFault,
}

impl ProbePort {
    /// Build an idle port that will answer from `plan` and report these values.
    fn from_plan(plan: Script, code: i32, reason: RepairableReason, fault: HostFault) -> Self {
        Self {
            plan,
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            preflights_at_last_run: 0,
            stale_runs: 0,
            last_preflight: LastPreflight::Unchecked,
            code,
            reason,
            fault,
        }
    }

    /// Build a port from symbolic choices.
    fn new() -> Self { Self::from_plan(Script::symbolic(), kani::any(), any_reason(), any_fault()) }

    /// Build a port with fixed answers, for the reachability harnesses.
    fn scripted(plan: Script) -> Self {
        Self::from_plan(plan, 0, RepairableReason::NotInstalled, HostFault::Io)
    }
}

impl CheckPorts for ProbePort {
    type Prepared = u8;

    /// Answer with the scripted readiness result and record it.
    fn preflight(&mut self) -> Preflight<u8> {
        let index = self.preflights.min(1);
        self.preflights += 1;
        match self.plan.answers[usize::from(index)] {
            Answer::Ready => {
                self.last_preflight = LastPreflight::Ready(index);
                Preflight::Ready(index)
            }
            Answer::Repairable => {
                self.last_preflight = LastPreflight::Repairable;
                Preflight::Repairable(self.reason)
            }
            Answer::Host => {
                self.last_preflight = LastPreflight::Host;
                Preflight::Host(self.fault)
            }
        }
    }

    /// Answer with the scripted provisioning result.
    fn repair(&mut self) -> Result<(), RepairFault> {
        self.repairs += 1;
        match self.plan.repair {
            RepairAnswer::Ok => Ok(()),
            RepairAnswer::Host => Err(RepairFault::Host(self.fault)),
            RepairAnswer::Failed => Err(RepairFault::Failed),
            RepairAnswer::Output => Err(RepairFault::Output),
        }
    }

    /// Answer with the scripted lint result, after requiring the prepared value
    /// to match the most recent ready check.
    fn run(&mut self, prepared: u8) -> Result<i32, RunFault> {
        if self.last_preflight != LastPreflight::Ready(prepared) {
            self.stale_runs += 1;
        }
        let kind = self.plan.runs[usize::from(self.runs.min(1))];
        self.runs += 1;
        self.prepared_from = prepared + 1;
        self.preflights_at_last_run = self.preflights;
        match kind {
            RunAnswer::Lint => Ok(self.code),
            RunAnswer::Provisioning => Err(RunFault::Provisioning(self.reason)),
            RunAnswer::Spawn => Err(RunFault::Spawn),
            RunAnswer::Output => Err(RunFault::Output),
            RunAnswer::Host => Err(RunFault::Host(self.fault)),
        }
    }
}

/// The outcome a failed repair reports for its scripted answer.
fn repair_failure_outcome(repair: RepairAnswer, fault: HostFault) -> CheckOutcome {
    match repair {
        // The successful answer is never a failure; a harness that assumes it
        // away still needs an arm, and the lint outcome is the honest one.
        RepairAnswer::Ok => CheckOutcome::Linted(0),
        RepairAnswer::Failed => CheckOutcome::RepairFailed,
        RepairAnswer::Output => CheckOutcome::OutputFailed,
        RepairAnswer::Host => CheckOutcome::HostFailed(fault),
    }
}

/// Run the flow against `port` with the given repair policy.
fn run_policy(port: &mut ProbePort, policy: Policy) -> CheckOutcome { run_check_flow(port, policy) }

/// Run the flow against a freshly scripted port, for the reachability proofs.
pub(super) fn run_scripted(plan: Script, policy: Policy) -> CheckOutcome {
    let mut port = ProbePort::scripted(plan);
    run_policy(&mut port, policy)
}

/// Run the flow against `port` and return it with its counters.
fn run_probe(mut port: ProbePort, policy: Policy) -> ProbePort {
    let _ = run_policy(&mut port, policy);
    port
}

/// Run a fresh symbolic port with no narrowing.
fn run_symbolic(policy: Policy) -> ProbePort { run_probe(ProbePort::new(), policy) }

/// Run a fresh symbolic port whose first answer is ready and lints.
fn run_ready_symbolic(policy: Policy) -> ProbePort {
    let port = ProbePort::new();
    kani::assume(port.plan.answers[0] == Answer::Ready && port.plan.runs[0] == RunAnswer::Lint);
    run_probe(port, policy)
}

/// Run a fresh symbolic port whose first answer is a host fault.
fn run_host_symbolic(policy: Policy) -> ProbePort {
    let port = ProbePort::new();
    kani::assume(port.plan.answers[0] == Answer::Host);
    run_probe(port, policy)
}

/// Check that a forbidden repair never provisions.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_refusal_is_honoured() {
    let port = run_symbolic(Policy {
        repair_allowed: false,
    });

    assert!(
        port.repairs == 0,
        "repair must not run when the policy forbids it"
    );
}

/// Check that a ready installation is linted once and never repaired.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_does_no_work_when_ready() {
    let port = run_ready_symbolic(any_policy());

    assert!(port.repairs == 0, "a ready installation is never repaired");
    assert!(
        port.runs == 1,
        "a ready installation is linted exactly once"
    );
}

/// Check that provisioning and readiness checks are bounded.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_repairs_at_most_once() {
    let port = run_symbolic(any_policy());

    assert!(port.repairs <= 1, "repair must run at most once");
    assert!(port.preflights <= 2, "the flow checks at most twice");
}

/// Check that every lint starts from the most recent ready check.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_runs_only_fresh_ready_preflight() {
    let port = run_symbolic(any_policy());

    if port.runs > 0 {
        assert!(port.runs <= 2, "the flow lints at most twice");
        assert!(
            port.stale_runs == 0,
            "every run must start from the latest ready readiness check"
        );
        assert!(
            port.prepared_from == port.preflights_at_last_run,
            "the last run must consume the readiness check that preceded it"
        );
        assert!(
            port.plan.answers[usize::from(port.prepared_from - 1)] == Answer::Ready,
            "the run must consume a ready readiness check"
        );
    }
}

/// Check that a first-readiness host fault never repairs or runs.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_host_faults_never_repair() {
    let port = run_host_symbolic(any_policy());

    assert!(port.repairs == 0, "a host fault is never repaired");
    assert!(port.runs == 0, "a host fault never reaches Dylint");
}

/// Check that a user-managed path's run fault is refused, never repaired.
///
/// The reason can only arise while the policy forbids repair, because a
/// user-managed path is what produces it, so this is the reachable pairing.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_refuses_a_user_path_run_fault() {
    let mut port = ProbePort::new();
    kani::assume(
        port.plan.answers[0] == Answer::Ready && port.plan.runs[0] == RunAnswer::Provisioning,
    );
    port.reason = RepairableReason::UserPath;

    let outcome = run_policy(
        &mut port,
        Policy {
            repair_allowed: false,
        },
    );

    assert!(
        port.repairs == 0,
        "a user-managed path must not be repaired"
    );
    assert!(
        outcome == CheckOutcome::Refused(RepairableReason::UserPath),
        "a user path that lost its suite must be refused, not reported as a repair that failed"
    );
}

/// Check that a failed repair is the outcome, with no second check or lint.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_failed_repair_stops_the_flow() {
    let mut port = ProbePort::new();
    kani::assume(port.plan.answers[0] == Answer::Repairable);
    kani::assume(port.plan.repair != RepairAnswer::Ok);
    let expected = repair_failure_outcome(port.plan.repair, port.fault);

    let outcome = run_policy(
        &mut port,
        Policy {
            repair_allowed: true,
        },
    );

    assert!(
        outcome == expected,
        "a failed repair must report the outcome its fault calls for"
    );
    assert!(
        port.preflights == 1,
        "a failed repair must not be followed by another readiness check"
    );
    assert!(port.runs == 0, "a failed repair must not reach Dylint");
}
