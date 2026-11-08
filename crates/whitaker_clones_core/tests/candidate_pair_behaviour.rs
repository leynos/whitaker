//! Behaviour-driven coverage for `CandidatePair::new`.
//!
//! Keep this harness in sync with `tests/features/candidate_pair.feature`.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{CandidatePair, FragmentId};
use whitaker_test_macros::allow_fixture_expansion_lints;

#[derive(Debug, Default)]
struct CandidatePairWorld {
    left: RefCell<Option<FragmentId>>,
    right: RefCell<Option<FragmentId>>,
    pair: RefCell<Option<CandidatePair>>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> CandidatePairWorld { CandidatePairWorld::default() }

fn with_pair(world: &CandidatePairWorld, assert_fn: impl FnOnce(&CandidatePair)) {
    let borrowed_pair = world.pair.borrow();
    let Some(candidate_pair) = borrowed_pair.as_ref() else {
        panic!("candidate pair must be present before running assertions");
    };
    assert_fn(candidate_pair);
}

#[given("input fragment IDs {left} and {right}")]
fn given_input_fragment_ids(world: &CandidatePairWorld, left: String, right: String) {
    *world.left.borrow_mut() = Some(FragmentId::from(left));
    *world.right.borrow_mut() = Some(FragmentId::from(right));
}

#[when("the candidate pair constructor is called")]
fn when_candidate_pair_constructor_is_called(world: &CandidatePairWorld) -> Result<(), String> {
    let left = world
        .left
        .borrow()
        .clone()
        .ok_or_else(|| "left fragment ID must be set before construction".to_owned())?;
    let right = world
        .right
        .borrow()
        .clone()
        .ok_or_else(|| "right fragment ID must be set before construction".to_owned())?;

    *world.pair.borrow_mut() = CandidatePair::new(left, right);
    Ok(())
}

#[then("the canonical pair is {left} and {right}")]
fn then_the_canonical_pair_is(world: &CandidatePairWorld, left: String, right: String) {
    with_pair(world, |pair| {
        assert_eq!(
            (pair.left().as_str(), pair.right().as_str()),
            (left.as_str(), right.as_str()),
            "canonical pair ordering must match the scenario expectation"
        );
    });
}

#[then("no candidate pair is returned")]
fn then_no_candidate_pair_is_returned(world: &CandidatePairWorld) {
    assert!(
        world.pair.borrow().is_none(),
        "no candidate pair must be returned for identical fragment IDs"
    );
}

#[scenario(path = "tests/features/candidate_pair.feature", index = 0)]
fn scenario_ordered_distinct_ids(world: CandidatePairWorld) { let _ = world; }

#[scenario(path = "tests/features/candidate_pair.feature", index = 1)]
fn scenario_reversed_distinct_ids(world: CandidatePairWorld) { let _ = world; }

#[scenario(path = "tests/features/candidate_pair.feature", index = 2)]
fn scenario_identical_ids(world: CandidatePairWorld) { let _ = world; }

#[scenario(path = "tests/features/candidate_pair.feature", index = 3)]
fn scenario_lexical_order_edge_case(world: CandidatePairWorld) { let _ = world; }
