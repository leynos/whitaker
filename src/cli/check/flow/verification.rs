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
    /// Whether provisioning succeeds.
    repair_ok: bool,
    /// Which run result the port reports.
    run_kind: u8,
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
            repair_ok: kani::any(),
            run_kind: kani::any(),
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
    fn scripted(answers: [u8; 2], repair_ok: bool, run_kind: u8) -> Self {
        Self {
            answers,
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            repair_ok,
            run_kind,
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
            READY => Preflight::Ready(index),
            REPAIRABLE => Preflight::Repairable(self.reason),
            _ => Preflight::Host(self.fault),
        }
    }

    fn repair(&mut self) -> Result<(), RepairFault> {
        self.repairs += 1;
        if self.repair_ok {
            Ok(())
        } else {
            Err(RepairFault::Failed)
        }
    }

    fn run(&mut self, prepared: u8) -> Result<i32, RunFault> {
        self.runs += 1;
        self.prepared_from = prepared + 1;
        match self.run_kind {
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

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_refusal_is_honoured() {
    let mut port = ProbePort::new();

    let _ = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: false,
        },
    );

    assert!(
        port.repairs == 0,
        "repair must not run when the policy forbids it"
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_does_no_work_when_ready() {
    let mut port = ProbePort::new();
    kani::assume(port.answers[0] == READY);

    let _ = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: kani::any(),
        },
    );

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

    let _ = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: kani::any(),
        },
    );

    assert!(port.repairs <= 1, "repair must run at most once");
    assert!(port.preflights <= 2, "the flow checks at most twice");
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_check_flow_runs_only_fresh_ready_preflight() {
    let mut port = ProbePort::new();

    let _ = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: kani::any(),
        },
    );

    if port.runs > 0 {
        assert!(port.runs == 1, "the flow lints at most once");
        assert!(
            port.prepared_from == port.preflights,
            "the run must consume the most recent readiness check"
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

    let _ = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: kani::any(),
        },
    );

    assert!(port.repairs == 0, "a host fault is never repaired");
    assert!(port.runs == 0, "a host fault never reaches Dylint");
}

#[kani::proof]
fn verify_linted_after_repair_is_reachable() {
    let mut port = ProbePort::scripted([REPAIRABLE, READY], true, 0);

    let outcome = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: true,
        },
    );

    assert!(outcome == CheckOutcome::Linted(0));
    assert!(false, "linting after a repair should have a counterexample");
}

#[kani::proof]
fn verify_not_converged_is_reachable() {
    let mut port = ProbePort::scripted([REPAIRABLE, REPAIRABLE], true, 0);

    let outcome = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: true,
        },
    );

    assert!(outcome == CheckOutcome::NotConverged(RepairableReason::NotInstalled));
    assert!(
        false,
        "a non-converging repair should have a counterexample"
    );
}

#[kani::proof]
fn verify_refused_is_reachable() {
    let mut port = ProbePort::scripted([REPAIRABLE, READY], true, 0);

    let outcome = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: false,
        },
    );

    assert!(outcome == CheckOutcome::Refused(RepairableReason::NotInstalled));
    assert!(false, "a refusal should have a counterexample");
}

#[kani::proof]
fn verify_host_failed_is_reachable() {
    let mut port = ProbePort::scripted([2, READY], true, 0);

    let outcome = run_check_flow(
        &mut port,
        Policy {
            repair_allowed: true,
        },
    );

    assert!(outcome == CheckOutcome::HostFailed(HostFault::Io));
    assert!(false, "a host failure should have a counterexample");
}
