//! Behaviour-driven coverage for module documentation detection.
//!
//! These scenarios exercise `detect_module_docs` to ensure modules only pass
//! when they begin with an inner doc comment.

use super::{ModuleDocDisposition, detect_module_docs, test_support::StubAttribute};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct ModuleWorld {
    attributes: RefCell<Vec<StubAttribute>>,
    result: RefCell<Option<ModuleDocDisposition>>,
}

impl ModuleWorld {
    fn push(&self, attribute: StubAttribute) {
        self.attributes.borrow_mut().push(attribute);
    }

    fn evaluate(&self) {
        let attrs = self.attributes.borrow();
        let outcome = detect_module_docs(attrs.as_slice());
        self.result.replace(Some(outcome));
    }

    fn outcome(&self) -> ModuleDocDisposition {
        *self
            .result
            .borrow()
            .as_ref()
            .expect("detector outcome should be recorded")
    }
}

#[fixture]
fn world() -> ModuleWorld {
    ModuleWorld::default()
}

#[given("the module begins with an inner doc comment")]
fn given_inner_doc(world: &ModuleWorld) {
    world.push(StubAttribute::inner_doc());
}

#[given("the module body starts with code only")]
fn given_no_attributes(world: &ModuleWorld) {
    let _ = world;
    // No attributes are added so the detector sees an empty list.
}

#[given("the module contains an inner configuration attribute")]
fn given_inner_allow(world: &ModuleWorld) {
    world.push(StubAttribute::inner_allow());
}

#[given("documentation follows that attribute")]
fn given_doc_after(world: &ModuleWorld) {
    world.push(StubAttribute::inner_doc());
}

#[given("the module declares only outer documentation")]
fn given_outer_doc(world: &ModuleWorld) {
    world.push(StubAttribute::outer_doc());
}

#[when("I validate the module documentation requirements")]
fn when_detect(world: &ModuleWorld) {
    world.evaluate();
}

#[then("the module is accepted")]
fn then_accept(world: &ModuleWorld) {
    assert_eq!(world.outcome(), ModuleDocDisposition::HasLeadingDoc);
}

#[then("documentation is reported missing")]
fn then_missing(world: &ModuleWorld) {
    assert_eq!(world.outcome(), ModuleDocDisposition::MissingDocs);
}

#[then("documentation is reported after other attributes")]
fn then_misordered(world: &ModuleWorld) {
    assert!(matches!(
        world.outcome(),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[scenario(path = "tests/features/module_docs.feature", index = 0)]
fn scenario_docs_first(world: ModuleWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/module_docs.feature", index = 1)]
fn scenario_missing_docs(world: ModuleWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/module_docs.feature", index = 2)]
fn scenario_misordered_docs(world: ModuleWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/module_docs.feature", index = 3)]
fn scenario_outer_docs(world: ModuleWorld) {
    let _ = world;
}
