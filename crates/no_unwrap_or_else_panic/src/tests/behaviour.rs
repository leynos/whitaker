//! Behaviour-driven coverage for lint decision logic.

use crate::context::ContextSummary;
use crate::policy::{LintPolicy, should_flag};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::Cell;

#[derive(Default)]
struct DecisionWorld {
    summary: Cell<ContextSummary>,
    allow_in_main: Cell<bool>,
    closure_panics: Cell<bool>,
    is_doctest: Cell<bool>,
    should_flag: Cell<Option<bool>>,
}

impl DecisionWorld {
    fn evaluate(&self) -> bool {
        let policy = LintPolicy::new(self.allow_in_main.get());
        should_flag(
            &policy,
            &self.summary.get(),
            self.closure_panics.get(),
            self.is_doctest.get(),
        )
    }
}

#[fixture]
fn world() -> DecisionWorld {
    DecisionWorld::default()
}

#[given("a panicking unwrap_or_else fallback outside tests")]
fn given_panicking(world: &DecisionWorld) {
    let mut summary = world.summary.get();
    summary.is_test = false;
    world.summary.set(summary);
    world.closure_panics.set(true);
}

#[given("a panicking unwrap_or_else fallback")]
fn given_panicking_alias(world: &DecisionWorld) {
    given_panicking(world);
}

#[given("code runs inside a test")]
fn given_test_context(world: &DecisionWorld) {
    let mut summary = world.summary.get();
    summary.is_test = true;
    world.summary.set(summary);
}

#[given("code runs inside main")]
fn given_main(world: &DecisionWorld) {
    let mut summary = world.summary.get();
    summary.in_main = true;
    world.summary.set(summary);
}

#[given("allow in main is enabled")]
fn given_allow_main(world: &DecisionWorld) {
    world.allow_in_main.set(true);
}

#[given("the fallback is safe")]
fn given_safe_fallback(world: &DecisionWorld) {
    world.closure_panics.set(false);
}

#[given("a doctest harness is active")]
fn given_doctest(world: &DecisionWorld) {
    world.is_doctest.set(true);
}

#[when("the lint policy is evaluated")]
fn when_policy_evaluated(world: &DecisionWorld) {
    world.should_flag.set(Some(world.evaluate()));
}

#[then("the lint triggers")]
fn then_triggers(world: &DecisionWorld) {
    assert_eq!(world.should_flag.get(), Some(true));
}

#[then("the lint is skipped")]
fn then_skipped(world: &DecisionWorld) {
    assert_eq!(world.should_flag.get(), Some(false));
}

#[scenario(path = "tests/features/policy.feature", index = 0)]
fn scenario_panicking_outside_tests(world: DecisionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/policy.feature", index = 1)]
fn scenario_panicking_inside_test(world: DecisionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/policy.feature", index = 2)]
fn scenario_panicking_in_main_with_allow(world: DecisionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/policy.feature", index = 3)]
fn scenario_safe_fallback(world: DecisionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/policy.feature", index = 4)]
fn scenario_doctest(world: DecisionWorld) {
    let _ = world;
}
