//! Behaviour-driven coverage for predicate branch evaluation logic.

use super::{ConditionDisposition, evaluate_condition};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::Cell;

#[derive(Default)]
struct PredicateWorld {
    limit: Cell<usize>,
    branches: Cell<usize>,
    disposition: Cell<Option<ConditionDisposition>>,
}

impl PredicateWorld {
    fn set_limit(&self, value: usize) {
        self.limit.set(value);
    }

    fn set_branches(&self, value: usize) {
        self.branches.set(value);
    }

    fn evaluate(&self) {
        let outcome = evaluate_condition(self.branches.get(), self.limit.get());
        self.disposition.set(Some(outcome));
    }

    fn disposition(&self) -> ConditionDisposition {
        self.disposition
            .get()
            .expect("predicate disposition must be recorded")
    }
}

#[fixture]
fn world() -> PredicateWorld {
    PredicateWorld::default()
}

#[given("the branch limit is {limit}")]
fn given_limit(world: &PredicateWorld, limit: usize) {
    world.set_limit(limit);
}

#[given("the predicate declares {branches} branches")]
fn given_branches(world: &PredicateWorld, branches: usize) {
    world.set_branches(branches);
}

#[when("I evaluate the predicate complexity")]
fn when_evaluate(world: &PredicateWorld) {
    world.evaluate();
}

#[then("the predicate is accepted")]
fn then_accepted(world: &PredicateWorld) {
    assert_eq!(world.disposition(), ConditionDisposition::WithinLimit);
}

#[then("the predicate is rejected")]
fn then_rejected(world: &PredicateWorld) {
    assert_eq!(world.disposition(), ConditionDisposition::ExceedsLimit);
}

#[scenario(path = "tests/features/conditional_branches.feature", index = 0)]
fn scenario_within_limit(world: PredicateWorld) { let _ = world; }

#[scenario(path = "tests/features/conditional_branches.feature", index = 1)]
fn scenario_at_limit(world: PredicateWorld) { let _ = world; }

#[scenario(path = "tests/features/conditional_branches.feature", index = 2)]
fn scenario_exceeds_limit(world: PredicateWorld) { let _ = world; }

#[scenario(path = "tests/features/conditional_branches.feature", index = 3)]
fn scenario_custom_limit(world: PredicateWorld) { let _ = world; }
