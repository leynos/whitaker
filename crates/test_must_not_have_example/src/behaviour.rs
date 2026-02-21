//! Behaviour-driven coverage for documentation example heuristics.

use crate::heuristics::{DocExampleViolation, detect_example_violation};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct DocumentationWorld {
    lines: RefCell<Vec<String>>,
    outcome: RefCell<Option<DocExampleViolation>>,
}

impl DocumentationWorld {
    fn push_line(&self, line: &str) {
        self.lines.borrow_mut().push(line.to_string());
    }

    fn evaluate(&self) {
        let doc = self.lines.borrow().join("\n");
        self.outcome.replace(detect_example_violation(&doc));
    }

    fn outcome(&self) -> Option<DocExampleViolation> {
        *self.outcome.borrow()
    }
}

#[fixture]
fn world() -> DocumentationWorld {
    DocumentationWorld::default()
}

#[given("documentation line {line}")]
fn given_line(world: &DocumentationWorld, line: String) {
    world.push_line(line.trim_matches('"'));
}

#[when("I evaluate the documentation")]
fn when_evaluate(world: &DocumentationWorld) {
    world.evaluate();
}

#[then("the violation is examples heading")]
fn then_examples_heading(world: &DocumentationWorld) {
    assert_eq!(world.outcome(), Some(DocExampleViolation::ExamplesHeading));
}

#[then("the violation is code fence")]
fn then_code_fence(world: &DocumentationWorld) {
    assert_eq!(world.outcome(), Some(DocExampleViolation::CodeFence));
}

#[then("there is no violation")]
fn then_no_violation(world: &DocumentationWorld) {
    assert_eq!(world.outcome(), None);
}

#[scenario(path = "tests/features/doc_examples.feature", index = 0)]
fn scenario_examples_heading(world: DocumentationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/doc_examples.feature", index = 1)]
fn scenario_code_fence(world: DocumentationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/doc_examples.feature", index = 2)]
fn scenario_inline_ticks(world: DocumentationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/doc_examples.feature", index = 3)]
fn scenario_plain_prose(world: DocumentationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/doc_examples.feature", index = 4)]
fn scenario_source_order(world: DocumentationWorld) {
    let _ = world;
}
