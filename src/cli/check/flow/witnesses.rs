//! Reachability witnesses for the check flow's outcomes.
//!
//! Each harness drives a fully scripted port to one outcome, asserts it got
//! there, then fails deliberately so Kani reports the reached state as a
//! witness rather than a proof. `scripts/run-kani.sh` matches the assertion
//! descriptions, so the messages here are part of the gate contract.

use super::verification::{HOST, READY, REPAIR_OK, REPAIRABLE, run_scripted};
use crate::cli::check::{
    classify::{HostFault, RepairableReason},
    exit::CheckOutcome,
};

/// A fully scripted flow the reachability proofs drive.
struct Scenario {
    /// The answer each readiness check gives, by call index.
    answers: [u8; 2],
    /// Which result the repair reports.
    repair: u8,
    /// Which run result the port reports, by call index.
    run_kinds: [u8; 2],
    /// Whether provisioning is permitted.
    repair_allowed: bool,
}

impl Scenario {
    /// Run the flow against this scenario and return the outcome it reached.
    fn run(&self) -> CheckOutcome {
        run_scripted(
            self.answers,
            self.repair,
            self.run_kinds,
            self.repair_allowed,
        )
    }
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
        answers: [REPAIRABLE, READY],
        repair: REPAIR_OK,
        run_kinds: [0, 0],
        repair_allowed: true,
    },
    CheckOutcome::Linted(0),
    "a repaired installation must lint",
    "linting after a repair should have a counterexample",
);

witness_harness!(
    verify_linted_after_a_run_failure_is_reachable,
    "Witness that a run-time provisioning failure can still lint after a repair.",
    Scenario {
        answers: [READY, READY],
        repair: REPAIR_OK,
        run_kinds: [1, 0],
        repair_allowed: true,
    },
    CheckOutcome::Linted(0),
    "a provisioning failure during the run must still lint after a repair",
    "linting after a run-time provisioning failure should have a counterexample",
);

witness_harness!(
    verify_not_converged_is_reachable,
    "Witness that a repair which leaves the installation unready is reported.",
    Scenario {
        answers: [REPAIRABLE, REPAIRABLE],
        repair: REPAIR_OK,
        run_kinds: [0, 0],
        repair_allowed: true,
    },
    CheckOutcome::NotConverged(RepairableReason::NotInstalled),
    "a repair that leaves the installation unready must not converge",
    "a non-converging repair should have a counterexample",
);

witness_harness!(
    verify_refused_is_reachable,
    "Witness that a forbidden repair is refused.",
    Scenario {
        answers: [REPAIRABLE, READY],
        repair: REPAIR_OK,
        run_kinds: [0, 0],
        repair_allowed: false,
    },
    CheckOutcome::Refused(RepairableReason::NotInstalled),
    "a forbidden repair must be refused",
    "a refusal should have a counterexample",
);

witness_harness!(
    verify_host_failed_is_reachable,
    "Witness that a host fault fails the flow.",
    Scenario {
        answers: [HOST, READY],
        repair: REPAIR_OK,
        run_kinds: [0, 0],
        repair_allowed: true,
    },
    CheckOutcome::HostFailed(HostFault::Io),
    "a host fault must fail the flow",
    "a host failure should have a counterexample",
);
