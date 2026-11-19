//! Exhaustive checks of the real flow over a finite set of port answers.

use super::{
    CheckPorts,
    Policy,
    RepairFault,
    RunFault,
    run_check_flow,
    samplers::{any_fault, any_reason},
};
use crate::cli::check::{
    classify::{HostFault, Preflight, RepairableReason},
    exit::CheckOutcome,
};

/// The answer a scripted readiness check gives.
pub(super) const READY: u8 = 0;
/// The answer a scripted readiness check gives for a repairable defect.
pub(super) const REPAIRABLE: u8 = 1;
/// The first answer that reports a host fault.
pub(super) const HOST: u8 = 2;
/// The answer a scripted repair gives when it succeeds.
pub(super) const REPAIR_OK: u8 = 0;
/// The answer a scripted repair gives when it reports a host fault.
const REPAIR_HOST: u8 = 2;

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
/// It counts its calls, and remembers which preflight a run consumed, so
/// the invariants can be stated over the calls rather than over outcomes.
struct ProbePort {
    /// The answer each readiness check gives, by call index.
    answers: [u8; 2],
    /// How many readiness checks have been answered.
    preflights: u8,
    /// How many provisioning attempts were requested.
    repairs: u8,
    /// How many lint runs were requested.
    runs: u8,
    /// One more than the preflight index the run consumed, or zero.
    prepared_from: u8,
    /// How many runs consumed something other than the latest ready preflight.
    stale_runs: u8,
    /// The result of the most recent readiness check.
    last_preflight: LastPreflight,
    /// Which result the repair reports (0 succeeds, 1 fails, 2 host fault).
    repair: u8,
    /// Which run result the port reports, by call index.
    run_kinds: [u8; 2],
    /// The status a successful run reports.
    code: i32,
    /// The repairable reason reported.
    reason: RepairableReason,
    /// The host fault reported.
    fault: HostFault,
}

impl ProbePort {
    /// Build a port from symbolic choices.
    fn new() -> Self {
        let second: u8 = kani::any();
        let third: u8 = kani::any();
        Self {
            answers: [kani::any(), second],
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            stale_runs: 0,
            last_preflight: LastPreflight::Unchecked,
            repair: kani::any(),
            run_kinds: [kani::any(), kani::any()],
            code: kani::any(),
            reason: any_reason(),
            fault: any_fault(),
        }
        .with_third_answer(third)
    }

    /// Keep the third symbolic answer meaningful for three-way choices.
    fn with_third_answer(mut self, third: u8) -> Self {
        if self.answers[1] < 2 {
            self.answers[1] = third.min(2);
        }
        self
    }

    /// Build a port with fixed answers, for the reachability harnesses.
    fn scripted(answers: [u8; 2], repair: u8, run_kinds: [u8; 2]) -> Self {
        Self {
            answers,
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            stale_runs: 0,
            last_preflight: LastPreflight::Unchecked,
            repair,
            run_kinds,
            code: 0,
            reason: RepairableReason::NotInstalled,
            fault: HostFault::Io,
        }
    }
}

impl CheckPorts for ProbePort {
    type Prepared = u8;

    /// Answer with the scripted readiness result and record it.
    fn preflight(&mut self) -> Preflight<u8> {
        let index = self.preflights.min(1);
        self.preflights += 1;
        match self.answers[usize::from(index)] {
            READY => {
                self.last_preflight = LastPreflight::Ready(index);
                Preflight::Ready(index)
            }
            REPAIRABLE => {
                self.last_preflight = LastPreflight::Repairable;
                Preflight::Repairable(self.reason)
            }
            _ => {
                self.last_preflight = LastPreflight::Host;
                Preflight::Host(self.fault)
            }
        }
    }

    /// Answer with the scripted provisioning result.
    fn repair(&mut self) -> Result<(), RepairFault> {
        self.repairs += 1;
        match self.repair {
            0 => Ok(()),
            REPAIR_HOST => Err(RepairFault::Host(self.fault)),
            _ => Err(RepairFault::Failed),
        }
    }

    /// Answer with the scripted lint result, after requiring the prepared value
    /// to match the most recent ready check.
    fn run(&mut self, prepared: u8) -> Result<i32, RunFault> {
        if self.last_preflight != LastPreflight::Ready(prepared) {
            self.stale_runs += 1;
        }
        let kind = self.run_kinds[usize::from(self.runs.min(1))];
        self.runs += 1;
        self.prepared_from = prepared + 1;
        match kind {
            0 => Ok(self.code),
            1 => Err(RunFault::Provisioning(self.reason)),
            2 => Err(RunFault::Spawn),
            3 => Err(RunFault::Output),
            _ => Err(RunFault::Host(self.fault)),
        }
    }
}

/// The outcome a failed repair reports for its scripted answer.
fn repair_failure_outcome(repair: u8, fault: HostFault) -> CheckOutcome {
    if repair == REPAIR_HOST {
        CheckOutcome::HostFailed(fault)
    } else {
        CheckOutcome::RepairFailed
    }
}

/// Run the flow against `port` with the given repair policy.
fn run_policy(port: &mut ProbePort, repair_allowed: bool) -> CheckOutcome {
    run_check_flow(port, Policy { repair_allowed })
}

/// Run the flow against a freshly scripted port, for the reachability proofs.
pub(super) fn run_scripted(
    answers: [u8; 2],
    repair: u8,
    run_kinds: [u8; 2],
    repair_allowed: bool,
) -> CheckOutcome {
    let mut port = ProbePort::scripted(answers, repair, run_kinds);
    run_policy(&mut port, repair_allowed)
}

/// Run the flow against `port` and return it with its counters.
fn run_probe(mut port: ProbePort, repair_allowed: bool) -> ProbePort {
    let _ = run_policy(&mut port, repair_allowed);
    port
}

/// Run a fresh symbolic port with no narrowing.
fn run_symbolic(repair_allowed: bool) -> ProbePort { run_probe(ProbePort::new(), repair_allowed) }

/// Run a fresh symbolic port whose first answer is ready and lints.
fn run_ready_symbolic(repair_allowed: bool) -> ProbePort {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] == READY && port.run_kinds[0] == 0);
    run_probe(port, repair_allowed)
}

/// Run a fresh symbolic port whose first answer is a host fault.
fn run_host_symbolic(repair_allowed: bool) -> ProbePort {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] >= HOST);
    run_probe(port, repair_allowed)
}

/// Check that a forbidden repair never provisions.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_refusal_is_honoured() {
    let port = run_symbolic(false);

    assert!(
        port.repairs == 0,
        "repair must not run when the policy forbids it"
    );
}

/// Check that a ready installation is linted once and never repaired.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_does_no_work_when_ready() {
    let port = run_ready_symbolic(kani::any());

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
    let port = run_symbolic(kani::any());

    assert!(port.repairs <= 1, "repair must run at most once");
    assert!(port.preflights <= 2, "the flow checks at most twice");
}

/// Check that every lint starts from the most recent ready check.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_runs_only_fresh_ready_preflight() {
    let port = run_symbolic(kani::any());

    if port.runs > 0 {
        assert!(port.runs <= 2, "the flow lints at most twice");
        assert!(
            port.stale_runs == 0,
            "every run must start from the latest ready readiness check"
        );
        assert!(
            port.prepared_from == port.preflights,
            "the last run must consume the most recent readiness check"
        );
        assert!(
            port.answers[usize::from(port.prepared_from - 1)] == READY,
            "the run must consume a ready readiness check"
        );
    }
}

/// Check that a first-readiness host fault never repairs or runs.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_host_faults_never_repair() {
    let port = run_host_symbolic(kani::any());

    assert!(port.repairs == 0, "a host fault is never repaired");
    assert!(port.runs == 0, "a host fault never reaches Dylint");
}

/// Check that a failed repair is the outcome, with no second check or lint.
#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_failed_repair_stops_the_flow() {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] == REPAIRABLE && port.repair != REPAIR_OK);
    let expected = repair_failure_outcome(port.repair, port.fault);

    let outcome = run_policy(&mut port, true);

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
