//! The decision table over every readiness class and both policies.

use rstest::rstest;

use super::{Policy, Step, decide};
use crate::ports::{HostFault, Preflight, RepairableReason};

/// The action a decision took, without its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Run,
    Repair,
    Refuse,
    Fail,
}

/// Report which action `step` is.
fn action<P>(step: &Step<P>) -> Action {
    match step {
        Step::Run(_) => Action::Run,
        Step::Repair(_) => Action::Repair,
        Step::Refuse(_) => Action::Refuse,
        Step::Fail(_) => Action::Fail,
    }
}

/// Check the action each readiness class takes under both policies.
#[rstest]
#[case::ready_allows_run(Preflight::Ready(()), true, Action::Run)]
#[case::ready_ignores_the_policy(Preflight::Ready(()), false, Action::Run)]
#[case::repairable_allows_repair(
    Preflight::Repairable(RepairableReason::Snapshot),
    true,
    Action::Repair
)]
#[case::repairable_refuses_without_permission(
    Preflight::Repairable(RepairableReason::Snapshot),
    false,
    Action::Refuse
)]
#[case::host_fault_fails_when_allowed(Preflight::Host(HostFault::Io), true, Action::Fail)]
#[case::host_fault_fails_when_forbidden(Preflight::Host(HostFault::Io), false, Action::Fail)]
fn decides_over_every_readiness_class(
    #[case] preflight: Preflight<()>,
    #[case] repair_allowed: bool,
    #[case] expected: Action,
) {
    let step = decide(preflight, Policy { repair_allowed });

    assert_eq!(
        action(&step),
        expected,
        "wrong decision for repair_allowed={repair_allowed}"
    );
}

/// Check the exact prepared value reaches the run.
#[test]
fn a_ready_installation_hands_its_own_prepared_value_to_the_run() {
    let step = decide(
        Preflight::Ready(41),
        Policy {
            repair_allowed: true,
        },
    );

    assert!(
        matches!(step, Step::Run(41)),
        "the prepared value must travel from the preflight to the run"
    );
}
