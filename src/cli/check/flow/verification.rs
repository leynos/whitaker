//! Exhaustive checks of the real flow over a finite set of port answers.

use super::{CheckPorts, Policy, RepairFault, RunFault, run_check_flow};
use crate::cli::check::{
    classify::{HostFault, Preflight, RepairableReason},
    exit::CheckOutcome,
};

/// The answer a scripted readiness check gives.
const READY: u8 = 0;
/// The answer a scripted readiness check gives for a repairable defect.
const REPAIRABLE: u8 = 1;
/// The index no readiness check has claimed, used before the first one.
const NO_PREFLIGHT: u8 = u8::MAX;
/// The answer a scripted repair gives when it succeeds.
const REPAIR_OK: u8 = 0;
/// The answer a scripted repair gives when it reports a host fault.
const REPAIR_HOST: u8 = 2;

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
    /// The index of the most recent ready preflight, or [`NO_PREFLIGHT`].
    last_ready: u8,
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
            last_ready: NO_PREFLIGHT,
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
            last_ready: NO_PREFLIGHT,
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

    fn preflight(&mut self) -> Preflight<u8> {
        let index = self.preflights.min(1);
        self.preflights += 1;
        match self.answers[usize::from(index)] {
            READY => {
                self.last_ready = index;
                Preflight::Ready(index)
            }
            REPAIRABLE => {
                self.last_ready = NO_PREFLIGHT;
                Preflight::Repairable(self.reason)
            }
            _ => {
                self.last_ready = NO_PREFLIGHT;
                Preflight::Host(self.fault)
            }
        }
    }

    fn repair(&mut self) -> Result<(), RepairFault> {
        self.repairs += 1;
        match self.repair {
            0 => Ok(()),
            REPAIR_HOST => Err(RepairFault::Host(self.fault)),
            _ => Err(RepairFault::Failed),
        }
    }

    fn run(&mut self, prepared: u8) -> Result<i32, RunFault> {
        let kind = self.run_kinds[usize::from(self.runs.min(1))];
        if prepared != self.last_ready {
            self.stale_runs += 1;
        }
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

/// Pick one of the seven repairable reasons.
fn any_reason() -> RepairableReason {
    match kani::any::<u8>() {
        0 => RepairableReason::NotInstalled,
        1 => RepairableReason::Metadata,
        2 => RepairableReason::LibrariesMissing,
        3 => RepairableReason::Snapshot,
        4 => RepairableReason::CargoDylint,
        5 => RepairableReason::DylintVersion,
        _ => RepairableReason::UserPath,
    }
}

/// Pick one of the four host faults.
fn any_fault() -> HostFault {
    match kani::any::<u8>() {
        0 => HostFault::DataDirectory,
        1 => HostFault::Io,
        2 => HostFault::Lock,
        _ => HostFault::Spawn,
    }
}

/// Run the flow against `port` with the given repair policy.
fn run_policy(port: &mut ProbePort, repair_allowed: bool) -> CheckOutcome {
    run_check_flow(port, Policy { repair_allowed })
}

/// Run the flow against a freshly scripted port, for the reachability proofs.
fn run_scripted(
    answers: [u8; 2],
    repair: u8,
    run_kinds: [u8; 2],
    repair_allowed: bool,
) -> CheckOutcome {
    let mut port = ProbePort::scripted(answers, repair, run_kinds);
    run_policy(&mut port, repair_allowed)
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_refusal_is_honoured() {
    let mut port = ProbePort::new();

    let _ = run_policy(&mut port, false);

    assert!(
        port.repairs == 0,
        "repair must not run when the policy forbids it"
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_does_no_work_when_ready() {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] == READY && port.run_kinds[0] == 0);

    let _ = run_policy(&mut port, kani::any());

    assert!(port.repairs == 0, "a ready installation is never repaired");
    assert!(
        port.runs == 1,
        "a ready installation is linted exactly once"
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_repairs_at_most_once() {
    let mut port = ProbePort::new();

    let _ = run_policy(&mut port, kani::any());

    assert!(port.repairs <= 1, "repair must run at most once");
    assert!(port.preflights <= 2, "the flow checks at most twice");
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_runs_only_fresh_ready_preflight() {
    let mut port = ProbePort::new();

    let _ = run_policy(&mut port, kani::any());

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

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_host_faults_never_repair() {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] >= 2);

    let _ = run_policy(&mut port, kani::any());

    assert!(port.repairs == 0, "a host fault is never repaired");
    assert!(port.runs == 0, "a host fault never reaches Dylint");
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_repair_host_fault_stops_the_flow() {
    let mut port = ProbePort::scripted([REPAIRABLE, READY], REPAIR_HOST, [0, 0]);

    let outcome = run_policy(&mut port, true);

    assert!(
        outcome == CheckOutcome::HostFailed(HostFault::Io),
        "a host fault during repair must be the outcome"
    );
    assert!(
        port.preflights == 1,
        "a failed repair must not be followed by another readiness check"
    );
    assert!(port.runs == 0, "a failed repair must not reach Dylint");
}

#[kani::proof]
fn verify_linted_after_repair_is_reachable() {
    let outcome = run_scripted([REPAIRABLE, READY], REPAIR_OK, [0, 0], true);

    assert!(outcome == CheckOutcome::Linted(0));
    assert!(false, "linting after a repair should have a counterexample");
}

#[kani::proof]
fn verify_not_converged_is_reachable() {
    let outcome = run_scripted([REPAIRABLE, REPAIRABLE], REPAIR_OK, [0, 0], true);

    assert!(outcome == CheckOutcome::NotConverged(RepairableReason::NotInstalled));
    assert!(
        false,
        "a non-converging repair should have a counterexample"
    );
}

#[kani::proof]
fn verify_refused_is_reachable() {
    let outcome = run_scripted([REPAIRABLE, READY], REPAIR_OK, [0, 0], false);

    assert!(outcome == CheckOutcome::Refused(RepairableReason::NotInstalled));
    assert!(false, "a refusal should have a counterexample");
}

#[kani::proof]
fn verify_host_failed_is_reachable() {
    let outcome = run_scripted([2, READY], REPAIR_OK, [0, 0], true);

    assert!(outcome == CheckOutcome::HostFailed(HostFault::Io));
    assert!(false, "a host failure should have a counterexample");
}

#[kani::proof]
fn verify_linted_after_a_run_failure_is_reachable() {
    let outcome = run_scripted([READY, READY], REPAIR_OK, [1, 0], true);

    assert!(outcome == CheckOutcome::Linted(0));
    assert!(
        false,
        "linting after a run-time provisioning failure should have a counterexample"
    );
}
