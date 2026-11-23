//! Behaviour-driven tests for the candidate-selection predicate.
//!
//! These scenarios are the readable contract examples from the ExecPlan. The
//! exhaustive table in `policy::selection_tests` is the coverage; this module
//! exists so a reader can see the intended behaviour stated in prose and check
//! it against the production predicate.

use std::cell::RefCell;

use googletest::prelude::{assert_that, eq};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::policy::{
    BodyPresence,
    CandidateFacts,
    Documentation,
    FunctionContext,
    HarnessStatus,
    PathSelection,
    Provenance,
    SkipReason,
    Verdict,
    select,
};

/// World state for one candidate-selection scenario.
///
/// The facts start from a module-level, source-authored, bodiless-free record
/// so each `given` step changes exactly the field it names.
struct SelectionWorld {
    facts: RefCell<CandidateFacts>,
    verdict: RefCell<Option<Verdict>>,
}

impl Default for SelectionWorld {
    fn default() -> Self {
        Self {
            facts: RefCell::new(CandidateFacts {
                harness: HarnessStatus::NotTestHarness,
                provenance: Provenance::Source,
                context: FunctionContext::Module,
                body: BodyPresence::Present,
                path: PathSelection::Selected,
                documentation: Documentation::Undocumented,
            }),
            verdict: RefCell::new(None),
        }
    }
}

impl SelectionWorld {
    /// Replaces one field of the candidate facts.
    fn update(&self, change: impl FnOnce(&mut CandidateFacts)) {
        change(&mut self.facts.borrow_mut());
    }
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> SelectionWorld { SelectionWorld::default() }

#[given("a module function in a selected source file")]
fn given_module_function(world: &SelectionWorld) {
    world.update(|facts| {
        facts.context = FunctionContext::Module;
        facts.path = PathSelection::Selected;
    });
}

#[given("an associated function inside a trait implementation")]
fn given_trait_impl_method(world: &SelectionWorld) {
    world.update(|facts| facts.context = FunctionContext::TraitImpl);
}

#[given("the compilation is a test harness")]
fn given_test_harness(world: &SelectionWorld) {
    world.update(|facts| facts.harness = HarnessStatus::TestHarness);
}

#[given("the compilation is not a test harness")]
fn given_not_a_test_harness(world: &SelectionWorld) {
    world.update(|facts| facts.harness = HarnessStatus::NotTestHarness);
}

#[given("the function has no documentation")]
fn given_no_documentation(world: &SelectionWorld) {
    world.update(|facts| facts.documentation = Documentation::Undocumented);
}

#[given("the function has a doc attribute with an include_str value")]
fn given_doc_attribute(world: &SelectionWorld) {
    // `#[doc = include_str!("...")]` is a name-value attribute whose value is
    // an unexpanded macro call, so its shape is `DocNameValue`.
    world.update(|facts| facts.documentation = Documentation::Documented);
}

#[when("the candidate predicate is evaluated")]
fn when_evaluated(world: &SelectionWorld) {
    let verdict = select(*world.facts.borrow());
    *world.verdict.borrow_mut() = Some(verdict);
}

#[then("the verdict is diagnose")]
fn then_diagnose(world: &SelectionWorld) {
    assert_that!(
        world.verdict.borrow().expect("the predicate must have run"),
        eq(Verdict::Diagnose)
    );
}

#[then("the verdict is skip because the function is documented")]
fn then_skip_documented(world: &SelectionWorld) {
    assert_that!(
        world.verdict.borrow().expect("the predicate must have run"),
        eq(Verdict::Skip(SkipReason::Documented))
    );
}

#[then("the verdict is skip because the function context is out of scope")]
fn then_skip_out_of_scope(world: &SelectionWorld) {
    assert_that!(
        world.verdict.borrow().expect("the predicate must have run"),
        eq(Verdict::Skip(SkipReason::OutOfScopeContext))
    );
}

#[scenario(path = "tests/features/candidate_selection.feature", index = 0)]
fn scenario_diagnoses_undocumented_module_function(world: SelectionWorld) { let _ = world; }

#[scenario(path = "tests/features/candidate_selection.feature", index = 1)]
fn scenario_doc_attribute_suppresses(world: SelectionWorld) { let _ = world; }

#[scenario(path = "tests/features/candidate_selection.feature", index = 2)]
fn scenario_trait_impl_is_out_of_scope(world: SelectionWorld) { let _ = world; }
