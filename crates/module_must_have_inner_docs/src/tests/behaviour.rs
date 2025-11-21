//! Behaviour-driven coverage for module documentation detection.
//!
//! These scenarios exercise the snippet classifier to ensure modules only pass
//! when they begin with an inner doc comment.

use super::{ModuleDocDisposition, detect_module_docs_from_snippet};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct ModuleWorld {
    prefix: RefCell<String>,
    result: RefCell<Option<ModuleDocDisposition>>,
}

impl ModuleWorld {
    fn push(&self, text: &str) {
        self.prefix.borrow_mut().push_str(text);
    }

    fn evaluate(&self) {
        let snippet = self.prefix.borrow();
        let outcome = detect_module_docs_from_snippet(&snippet);
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
    world.push("//! module docs\n");
}

#[given("the module body starts with code only")]
fn given_no_attributes(world: &ModuleWorld) {
    world.push("pub fn demo() {}\n");
}

#[given("the module contains an inner configuration attribute")]
fn given_inner_allow(world: &ModuleWorld) {
    world.push("#![allow(dead_code)]\n");
}

#[given("documentation follows that attribute")]
fn given_doc_after(world: &ModuleWorld) {
    world.push("//! trailing docs\n");
}

#[given("the module declares only outer documentation")]
fn given_outer_doc(world: &ModuleWorld) {
    world.push("/// outer docs\n");
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
