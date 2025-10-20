//! Behaviour-driven tests for context detection and test-like attribute recognition,
//! including configured additions to the recognised attribute set.

use common::attributes::{Attribute, AttributeKind, AttributePath};
use common::context::{ContextEntry, in_test_like_context_with, is_test_fn_with};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
struct FunctionFixture {
    attributes: RefCell<Vec<Attribute>>,
    context: RefCell<Vec<ContextEntry>>,
    additional: RefCell<Vec<AttributePath>>,
}

impl FunctionFixture {
    fn new() -> Self {
        Self {
            attributes: RefCell::new(Vec::new()),
            context: RefCell::new(vec![ContextEntry::function("demo", Vec::new())]),
            additional: RefCell::new(Vec::new()),
        }
    }

    fn push_attribute(&self, attribute: Attribute) {
        self.attributes.borrow_mut().push(attribute.clone());
        if let Some(entry) = self.context.borrow_mut().last_mut() {
            entry.push_attribute(attribute);
        }
    }

    fn clear(&self) {
        self.attributes.borrow_mut().clear();
        if let Some(entry) = self.context.borrow_mut().last_mut() {
            entry.attributes_mut().clear();
        }
        self.reset_additional();
    }

    fn reset_additional(&self) {
        self.additional.borrow_mut().clear();
    }

    fn attributes(&self) -> std::cell::Ref<'_, Vec<Attribute>> {
        self.attributes.borrow()
    }

    fn context(&self) -> std::cell::Ref<'_, Vec<ContextEntry>> {
        self.context.borrow()
    }

    fn additional(&self) -> std::cell::Ref<'_, Vec<AttributePath>> {
        self.additional.borrow()
    }

    fn configure_additional(&self, path: &str) {
        self.additional.borrow_mut().push(AttributePath::from(path));
    }
}

#[fixture]
fn function() -> FunctionFixture {
    FunctionFixture::new()
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Evaluation {
    is_test: bool,
    in_context: bool,
}

#[fixture]
fn evaluation() -> Evaluation {
    Evaluation::default()
}

#[given("a function annotated with rstest")]
fn given_rstest(function: &FunctionFixture) {
    let attribute = Attribute::new(AttributePath::from("rstest"), AttributeKind::Outer);
    function.push_attribute(attribute);
}

#[given("a function annotated with tokio::test")]
fn given_tokio(function: &FunctionFixture) {
    let attribute = Attribute::new(AttributePath::from("tokio::test"), AttributeKind::Outer);
    function.push_attribute(attribute);
}

#[given("a function without test attributes")]
fn given_plain(function: &FunctionFixture) {
    function.clear();
}

#[given("the lint recognises {path} as a test attribute")]
fn given_custom_attribute(function: &FunctionFixture, path: String) {
    function.configure_additional(&path);
}

#[given("a function annotated with the custom test attribute {path}")]
fn given_function_with_custom_attribute(function: &FunctionFixture, path: String) {
    let attribute = Attribute::new(AttributePath::from(path), AttributeKind::Outer);
    function.push_attribute(attribute);
}

#[when("I check whether the function is test-like")]
fn when_check(function: &FunctionFixture) -> Evaluation {
    let attributes = function.attributes();
    let context = function.context();
    let additional = function.additional();
    Evaluation {
        is_test: is_test_fn_with(attributes.as_slice(), additional.as_slice()),
        in_context: in_test_like_context_with(context.as_slice(), additional.as_slice()),
    }
}

#[then("the function is recognised as test-like")]
fn then_positive(evaluation: &Evaluation) {
    assert!(evaluation.is_test);
}

#[then("its context is marked as test-like")]
fn then_context_positive(evaluation: &Evaluation) {
    assert!(evaluation.in_context);
}

#[then("the function is recognised as not test-like")]
fn then_negative(evaluation: &Evaluation) {
    assert!(!evaluation.is_test);
}

#[then("its context is not marked as test-like")]
fn then_context_negative(evaluation: &Evaluation) {
    assert!(!evaluation.in_context);
}

#[scenario(path = "tests/features/context_detection.feature", index = 0)]
fn scenario_detects_rstest(function: FunctionFixture, evaluation: Evaluation) {
    let _ = (function, evaluation);
}

#[scenario(path = "tests/features/context_detection.feature", index = 1)]
fn scenario_detects_tokio(function: FunctionFixture, evaluation: Evaluation) {
    let _ = (function, evaluation);
}

#[scenario(path = "tests/features/context_detection.feature", index = 2)]
fn scenario_ignores_plain(function: FunctionFixture, evaluation: Evaluation) {
    let _ = (function, evaluation);
}

#[scenario(path = "tests/features/context_detection.feature", index = 3)]
fn scenario_recognises_custom(function: FunctionFixture, evaluation: Evaluation) {
    let _ = (function, evaluation);
}
