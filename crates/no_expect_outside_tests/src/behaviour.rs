use crate::context::{ContextSummary, summarise_context};
use common::attributes::{Attribute, AttributeKind, AttributePath};
use common::{ContextEntry, ContextKind};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct ContextWorld {
    entries: RefCell<Vec<ContextEntry>>,
    cfg_test: RefCell<bool>,
    summary: RefCell<ContextSummary>,
    additional: RefCell<Vec<AttributePath>>,
    is_doctest: RefCell<bool>,
    skip_lint: RefCell<bool>,
}

impl ContextWorld {
    fn push_function(&self, name: &str) {
        self.entries
            .borrow_mut()
            .push(ContextEntry::function(name, Vec::new()));
    }

    fn push_test_function(&self, name: &str) {
        self.entries.borrow_mut().push(ContextEntry::function(
            name,
            vec![Attribute::new(
                AttributePath::from("test"),
                AttributeKind::Outer,
            )],
        ));
    }

    fn push_module(&self, name: &str) {
        self.entries
            .borrow_mut()
            .push(ContextEntry::new(name, ContextKind::Module, Vec::new()));
    }

    fn enable_cfg_test(&self) {
        *self.cfg_test.borrow_mut() = true;
    }

    fn register_additional_attribute(&self, path: &str) {
        self.additional.borrow_mut().push(AttributePath::from(path));
    }

    fn mark_doctest(&self) {
        *self.is_doctest.borrow_mut() = true;
    }

    fn evaluate(&self) {
        let entries = self.entries.borrow();
        let summary = summarise_context(
            entries.as_slice(),
            *self.cfg_test.borrow(),
            self.additional.borrow().as_slice(),
        );
        *self.skip_lint.borrow_mut() = *self.is_doctest.borrow() || summary.is_test;
        *self.summary.borrow_mut() = summary;
    }

    fn summary(&self) -> ContextSummary {
        self.summary.borrow().clone()
    }

    fn should_skip_lint(&self) -> bool {
        *self.skip_lint.borrow()
    }
}

#[fixture]
fn world() -> ContextWorld {
    ContextWorld::default()
}

#[given("a non-test function named {name}")]
fn given_plain_function(world: &ContextWorld, name: String) {
    world.push_function(&name);
}

#[given("a test function named {name}")]
fn given_test_function(world: &ContextWorld, name: String) {
    world.push_test_function(&name);
}

#[given("a module with cfg(test)")]
fn given_cfg_test_module(world: &ContextWorld) {
    world.push_module("tests");
    world.enable_cfg_test();
}

#[given("an additional test attribute {path} is configured")]
fn given_additional_attribute(world: &ContextWorld, path: String) {
    world.register_additional_attribute(&path);
}

#[given("a function annotated with the additional attribute {path}")]
fn given_function_with_additional_attribute(world: &ContextWorld, path: String) {
    world.entries.borrow_mut().push(ContextEntry::function(
        "custom",
        vec![Attribute::new(
            AttributePath::from(path),
            AttributeKind::Outer,
        )],
    ));
}

#[given("the lint is running within a doctest")]
fn given_doctest(world: &ContextWorld) {
    world.mark_doctest();
}

#[when("I summarise the context")]
fn when_summarise(world: &ContextWorld) {
    world.evaluate();
}

#[then("the context is marked as production")]
fn then_production(world: &ContextWorld) {
    assert!(!world.summary().is_test);
}

#[then("the context is marked as test")]
fn then_test(world: &ContextWorld) {
    assert!(world.summary().is_test);
}

#[then("the function name is {expected}")]
fn then_function(world: &ContextWorld, expected: String) {
    assert_eq!(
        world.summary().function_name.as_deref(),
        Some(expected.as_str())
    );
}

#[then("no function name is recorded")]
fn then_no_function(world: &ContextWorld) {
    assert!(world.summary().function_name.is_none());
}

#[then("the lint is skipped")]
fn then_lint_skipped(world: &ContextWorld) {
    assert!(world.should_skip_lint());
}

#[scenario(path = "tests/features/context_summary.feature", index = 0)]
fn scenario_production(world: ContextWorld) {
    world.push_function("handler");
    world.evaluate();
    assert!(!world.summary().is_test);
    assert_eq!(world.summary().function_name.as_deref(), Some("handler"));
}

#[scenario(path = "tests/features/context_summary.feature", index = 1)]
fn scenario_test(world: ContextWorld) {
    world.push_test_function("works");
    world.evaluate();
    assert!(world.summary().is_test);
    assert_eq!(world.summary().function_name.as_deref(), Some("works"));
}

#[scenario(path = "tests/features/context_summary.feature", index = 2)]
fn scenario_cfg_test(world: ContextWorld) {
    world.push_module("tests");
    world.enable_cfg_test();
    world.evaluate();
    assert!(world.summary().is_test);
    assert!(world.summary().function_name.is_none());
}

#[scenario(path = "tests/features/context_summary.feature", index = 3)]
fn scenario_additional_attribute(world: ContextWorld) {
    world.register_additional_attribute("custom::test");
    world.entries.borrow_mut().push(ContextEntry::function(
        "custom",
        vec![Attribute::new(
            AttributePath::from("custom::test"),
            AttributeKind::Outer,
        )],
    ));
    world.evaluate();
    assert!(world.summary().is_test);
    assert_eq!(world.summary().function_name.as_deref(), Some("custom"));
}

#[scenario(path = "tests/features/context_summary.feature", index = 4)]
fn scenario_doctest(world: ContextWorld) {
    world.push_function("handler");
    world.mark_doctest();
    world.evaluate();
    assert!(world.should_skip_lint());
    assert_eq!(world.summary().function_name.as_deref(), Some("handler"));
}
