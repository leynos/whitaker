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

/// One scripted readiness answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// Dylint can run against this installation.
    Ready,
    /// Provisioning could make the installation usable.
    Repairable,
    /// The host failed before readiness could be decided.
    Host,
}

/// One scripted provisioning result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepairAnswer {
    /// Provisioning completed.
    Ok,
    /// Provisioning ran and failed.
    Failed,
    /// The host failed before provisioning could start.
    Host,
}

/// One scripted lint result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunAnswer {
    /// Dylint ran and reported a status.
    Lint,
    /// The installation stopped being ready before the run.
    Provisioning,
    /// `cargo dylint` could not be started.
    Spawn,
    /// Whitaker could not write its own output.
    Output,
    /// The host failed while Dylint was being started or run.
    Host,
}

/// Every answer a scripted port will give, in one value.
///
/// The flow's arguments are the port itself, the readiness answers, and the
/// policy; keeping the answers in one script means the helpers that build a
/// port take one argument instead of a list of primitives.
#[derive(Debug, Clone, Copy)]
pub(super) struct Script {
    /// The answer each readiness check gives, by call index.
    answers: [Answer; 2],
    /// Which result the repair reports.
    repair: RepairAnswer,
    /// Which result the run reports, by call index.
    runs: [RunAnswer; 2],
}

/// Assemble a scripted port's answers.
pub(super) const fn script(
    answers: [Answer; 2],
    repair: RepairAnswer,
    runs: [RunAnswer; 2],
) -> Script {
    Script {
        answers,
        repair,
        runs,
    }
}

impl Script {
    /// Build a script whose every answer is a symbolic choice.
    fn symbolic() -> Self {
        Self {
            answers: [any_answer(), any_answer()],
            repair: any_repair(),
            runs: [any_run(), any_run()],
        }
    }
}

/// Sample one readiness answer.
fn any_answer() -> Answer {
    match kani::any::<u8>() {
        0 => Answer::Ready,
        1 => Answer::Repairable,
        _ => Answer::Host,
    }
}

/// Sample one provisioning result.
fn any_repair() -> RepairAnswer {
    match kani::any::<u8>() {
        0 => RepairAnswer::Ok,
        2 => RepairAnswer::Host,
        _ => RepairAnswer::Failed,
    }
}

/// Sample one lint result.
fn any_run() -> RunAnswer {
    match kani::any::<u8>() {
        0 => RunAnswer::Lint,
        1 => RunAnswer::Provisioning,
        2 => RunAnswer::Spawn,
        3 => RunAnswer::Output,
        _ => RunAnswer::Host,
    }
}

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
/// It counts its calls, and remembers which preflight a run consumed, so
/// the invariants can be stated over the calls rather than over outcomes.
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
    /// Freshness is relative to the moment of the run, not to the end of the
    /// flow: a run-time provisioning failure spends the repair, and the check
    /// after that repair can still find the installation unready, so the flow
    /// legitimately ends with a readiness check later than the last run.
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
    /// Build a port from symbolic choices.
    fn new() -> Self {
        Self {
            plan: Script::symbolic(),
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            preflights_at_last_run: 0,
            stale_runs: 0,
            last_preflight: LastPreflight::Unchecked,
            code: kani::any(),
            reason: any_reason(),
            fault: any_fault(),
        }
    }

    /// Build a port with fixed answers, for the reachability harnesses.
    fn scripted(plan: Script) -> Self {
        Self {
            plan,
            preflights: 0,
            repairs: 0,
            runs: 0,
            prepared_from: 0,
            preflights_at_last_run: 0,
            stale_runs: 0,
            last_preflight: LastPreflight::Unchecked,
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
    if repair == RepairAnswer::Host {
        CheckOutcome::HostFailed(fault)
    } else {
        CheckOutcome::RepairFailed
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
