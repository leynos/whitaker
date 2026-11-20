//! Reachability witnesses for the check flow's outcomes.
//!
//! Each harness drives a fully scripted port to one outcome, asserts it got
//! there, then fails deliberately so Kani reports the reached state as a
//! witness rather than a proof. `scripts/run-kani.sh` matches the assertion
//! descriptions, so the messages here are part of the gate contract.
//!
//! Every outcome the flow can reach has a witness here: linting, a refusal, a
//! repair that failed, a repair that did not converge, a host fault, a run
//! that could not start, and a write of Whitaker's own output that failed.
//! [`CheckOutcome::ConfigurationFailed`] has none because no flow reaches it in
//! phase one; its arm is inhabited and mapped by the exit witnesses in
//! `exit.rs` instead.

use super::{
    Policy,
    script::{Answer, RepairAnswer, RunAnswer, Script, script},
    verification::run_scripted,
};
use crate::cli::check::{
    classify::{HostFault, RepairableReason},
    exit::CheckOutcome,
};

/// A fully scripted flow the reachability proofs drive.
struct Scenario {
    /// The answers the port gives.
    plan: Script,
    /// Whether provisioning is permitted.
    policy: Policy,
}

impl Scenario {
    /// Run the flow against this scenario and return the outcome it reached.
    fn run(&self) -> CheckOutcome { run_scripted(self.plan, self.policy) }
}

/// Define one reachability harness from a scripted scenario and the outcome it
/// must reach.
///
/// The witness and failure messages must stay literals at the call site: Kani
/// derives each check's description from them, and `scripts/run-kani.sh`
/// matches those descriptions to tell a valid witness from a broken one.
macro_rules! witness_harness {
    (
        $name:ident,
        $summary:literal,
        $scenario:expr,
        $expected:expr,
        $witness:literal,
        $failure:literal,
    ) => {
        #[doc = $summary]
        #[kani::proof]
        fn $name() {
            assert!($scenario.run() == $expected, $witness);
            assert!(false, $failure);
        }
    };
}

witness_harness!(
    verify_linted_after_repair_is_reachable,
    "Witness that a repaired installation can lint.",
    Scenario {
        plan: script(
            [Answer::Repairable, Answer::Ready],
            RepairAnswer::Ok,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::Linted(0),
    "a repaired installation must lint",
    "linting after a repair should have a counterexample",
);

witness_harness!(
    verify_linted_after_a_run_failure_is_reachable,
    "Witness that a run-time provisioning failure can still lint after a repair.",
    Scenario {
        plan: script(
            [Answer::Ready, Answer::Ready],
            RepairAnswer::Ok,
            [RunAnswer::Provisioning, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::Linted(0),
    "a provisioning failure during the run must still lint after a repair",
    "linting after a run-time provisioning failure should have a counterexample",
);

witness_harness!(
    verify_not_converged_is_reachable,
    "Witness that a repair which leaves the installation unready is reported.",
    Scenario {
        plan: script(
            [Answer::Repairable, Answer::Repairable],
            RepairAnswer::Ok,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::NotConverged(RepairableReason::NotInstalled),
    "a repair that leaves the installation unready must not converge",
    "a non-converging repair should have a counterexample",
);

witness_harness!(
    verify_refused_is_reachable,
    "Witness that a forbidden repair is refused.",
    Scenario {
        plan: script(
            [Answer::Repairable, Answer::Ready],
            RepairAnswer::Ok,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: false
        },
    },
    CheckOutcome::Refused(RepairableReason::NotInstalled),
    "a forbidden repair must be refused",
    "a refusal should have a counterexample",
);

witness_harness!(
    verify_repair_failed_is_reachable,
    "Witness that a repair which ran and failed is reported.",
    Scenario {
        plan: script(
            [Answer::Repairable, Answer::Ready],
            RepairAnswer::Failed,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::RepairFailed,
    "a repair that ran and failed must be reported",
    "a failed repair should have a counterexample",
);

witness_harness!(
    verify_run_failed_is_reachable,
    "Witness that a run which cannot start is reported.",
    Scenario {
        plan: script(
            [Answer::Ready, Answer::Ready],
            RepairAnswer::Ok,
            [RunAnswer::Spawn, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::RunFailed,
    "a run that cannot start must be reported",
    "a run failure should have a counterexample",
);

witness_harness!(
    verify_output_failed_is_reachable,
    "Witness that a repair whose notice could not be written is reported.",
    Scenario {
        plan: script(
            [Answer::Repairable, Answer::Ready],
            RepairAnswer::Output,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::OutputFailed,
    "a repair whose notice could not be written must be reported",
    "an output failure during repair should have a counterexample",
);

witness_harness!(
    verify_host_failed_is_reachable,
    "Witness that a host fault fails the flow.",
    Scenario {
        plan: script(
            [Answer::Host, Answer::Ready],
            RepairAnswer::Ok,
            [RunAnswer::Lint, RunAnswer::Lint],
        ),
        policy: Policy {
            repair_allowed: true
        },
    },
    CheckOutcome::HostFailed(HostFault::Io),
    "a host fault must fail the flow",
    "a host failure should have a counterexample",
);
