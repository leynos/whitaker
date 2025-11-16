//! Behaviour-driven coverage for predicate branch evaluation logic.

use super::{ConditionDisposition, evaluate_condition};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct PredicateWorld {
    limit: usize,
    branches: usize,
    disposition: Option<ConditionDisposition>,
}

impl PredicateWorld {
    fn set_limit(&mut self, value: usize) {
        self.limit = value;
    }

    fn set_branches(&mut self, value: usize) {
        self.branches = value;
    }

    fn evaluate(&mut self) {
        let outcome = evaluate_condition(self.branches, self.limit);
        self.disposition = Some(outcome);
    }

    fn disposition(&self) -> ConditionDisposition {
        self.disposition
            .expect("predicate disposition must be recorded")
    }
}

#[fixture]
fn world() -> PredicateWorld {
    PredicateWorld::default()
}

#[given("the branch limit is {limit}")]
fn given_limit(world: &mut PredicateWorld, limit: usize) {
    world.set_limit(limit);
}

#[given("the predicate declares {branches} branches")]
fn given_branches(world: &mut PredicateWorld, branches: usize) {
    world.set_branches(branches);
}

#[when("I evaluate the predicate complexity")]
fn when_evaluate(world: &mut PredicateWorld) {
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
fn scenario_within_limit(_world: PredicateWorld) {}

#[scenario(path = "tests/features/conditional_branches.feature", index = 1)]
fn scenario_at_limit(_world: PredicateWorld) {}

#[scenario(path = "tests/features/conditional_branches.feature", index = 2)]
fn scenario_exceeds_limit(_world: PredicateWorld) {}

#[scenario(path = "tests/features/conditional_branches.feature", index = 3)]
fn scenario_custom_limit(_world: PredicateWorld) {}
