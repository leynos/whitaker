//! Behaviour-driven coverage for cognitive complexity computation.

use common::brain_type_metrics::cognitive_complexity::CognitiveComplexityBuilder;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};

#[derive(Debug)]
struct CcWorld {
    builder: RefCell<Option<CognitiveComplexityBuilder>>,
    score_result: Cell<Option<usize>>,
}

impl Default for CcWorld {
    fn default() -> Self {
        Self {
            builder: RefCell::new(Some(CognitiveComplexityBuilder::new())),
            score_result: Cell::new(None),
        }
    }
}

#[fixture]
fn world() -> CcWorld {
    CcWorld::default()
}

// --- Helpers ---

/// Borrows the builder mutably and applies a closure to it.
fn with_builder(world: &CcWorld, f: impl FnOnce(&mut CognitiveComplexityBuilder)) {
    let mut slot = world.builder.borrow_mut();
    f(slot
        .as_mut()
        .unwrap_or_else(|| panic!("builder already consumed")));
}

// --- Given steps ---

#[given("a new complexity builder")]
fn given_new_builder(world: &CcWorld) {
    // The builder is created by Default; nothing to do.
    let _ = world;
}

#[given("a structural increment not from expansion")]
fn given_structural_not_expanded(world: &CcWorld) {
    with_builder(world, |b| b.record_structural_increment(false));
}

#[given("a structural increment from expansion")]
fn given_structural_expanded(world: &CcWorld) {
    with_builder(world, |b| b.record_structural_increment(true));
}

#[given("a nesting increment not from expansion")]
fn given_nesting_not_expanded(world: &CcWorld) {
    with_builder(world, |b| b.record_nesting_increment(false));
}

#[given("a fundamental increment not from expansion")]
fn given_fundamental_not_expanded(world: &CcWorld) {
    with_builder(world, |b| b.record_fundamental_increment(false));
}

#[given("nesting is pushed not from expansion")]
fn given_push_nesting_not_expanded(world: &CcWorld) {
    with_builder(world, |b| b.push_nesting(false));
}

#[given("nesting is pushed from expansion")]
fn given_push_nesting_expanded(world: &CcWorld) {
    with_builder(world, |b| b.push_nesting(true));
}

#[given("nesting is popped")]
fn given_pop_nesting(world: &CcWorld) {
    with_builder(world, |b| b.pop_nesting());
}

// --- When steps ---

#[when("the complexity is finalised")]
fn when_finalised(world: &CcWorld) {
    let builder = world
        .builder
        .borrow_mut()
        .take()
        .unwrap_or_else(|| panic!("builder already consumed"));
    world.score_result.set(Some(builder.build()));
}

// --- Then steps ---

#[then("the complexity score is {expected}")]
fn then_score_is(world: &CcWorld, expected: usize) {
    assert_eq!(world.score_result.get(), Some(expected));
}

// Scenario indices must match their declaration order in
// `tests/features/cognitive_complexity.feature`. Adding, removing, or
// reordering scenarios in the feature file requires updating the indices
// here.

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 0)]
fn scenario_empty_function(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 1)]
fn scenario_single_if(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 2)]
fn scenario_nested_if(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 3)]
fn scenario_macro_structural_excluded(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 4)]
fn scenario_macro_nesting_no_inflate(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 5)]
fn scenario_boolean_operators(world: CcWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cognitive_complexity.feature", index = 6)]
fn scenario_mixed_real_and_expansion(world: CcWorld) {
    let _ = world;
}
