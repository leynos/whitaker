//! Behaviour-driven coverage for method metadata extraction.

use common::lcom4::MethodInfo;
use common::lcom4::extract::MethodInfoBuilder;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Debug, Default)]
struct ExtractionWorld {
    builder: RefCell<Option<MethodInfoBuilder>>,
    result: RefCell<Option<MethodInfo>>,
}

impl ExtractionWorld {
    fn with_builder(&self, f: impl FnOnce(&mut MethodInfoBuilder)) {
        let mut slot = self.builder.borrow_mut();
        if let Some(builder) = slot.as_mut() {
            f(builder);
        }
    }

    fn build(&self) {
        if let Some(builder) = self.builder.borrow_mut().take() {
            self.result.replace(Some(builder.build()));
        }
    }

    fn has_field(&self, field: &str) -> bool {
        self.result
            .borrow()
            .as_ref()
            .is_some_and(|info| info.accessed_fields().contains(field))
    }

    fn has_call(&self, method: &str) -> bool {
        self.result
            .borrow()
            .as_ref()
            .is_some_and(|info| info.called_methods().contains(method))
    }

    fn fields_empty(&self) -> bool {
        self.result
            .borrow()
            .as_ref()
            .is_none_or(|info| info.accessed_fields().is_empty())
    }

    fn calls_empty(&self) -> bool {
        self.result
            .borrow()
            .as_ref()
            .is_none_or(|info| info.called_methods().is_empty())
    }
}

#[fixture]
fn world() -> ExtractionWorld {
    ExtractionWorld::default()
}

// --- Given steps ---

#[given("an extraction builder for method {name}")]
fn given_builder(world: &ExtractionWorld, name: String) {
    world.builder.replace(Some(MethodInfoBuilder::new(name)));
}

#[given("a field access to {field} not from expansion")]
fn given_field_not_expanded(world: &ExtractionWorld, field: String) {
    world.with_builder(|b| b.record_field_access(&field, false));
}

#[given("a field access to {field} from expansion")]
fn given_field_expanded(world: &ExtractionWorld, field: String) {
    world.with_builder(|b| b.record_field_access(&field, true));
}

#[given("a method call to {method} not from expansion")]
fn given_call_not_expanded(world: &ExtractionWorld, method: String) {
    world.with_builder(|b| b.record_method_call(&method, false));
}

#[given("a method call to {method} from expansion")]
fn given_call_expanded(world: &ExtractionWorld, method: String) {
    world.with_builder(|b| b.record_method_call(&method, true));
}

// --- When steps ---

#[when("I build the method info")]
fn when_build(world: &ExtractionWorld) {
    world.build();
}

// --- Then steps ---

#[then("the accessed fields contain {field}")]
fn then_fields_contain(world: &ExtractionWorld, field: String) {
    assert!(
        world.has_field(&field),
        "expected accessed_fields to contain '{field}'"
    );
}

#[then("the accessed fields do not contain {field}")]
fn then_fields_not_contain(world: &ExtractionWorld, field: String) {
    assert!(
        !world.has_field(&field),
        "expected accessed_fields NOT to contain '{field}'"
    );
}

#[then("the called methods contain {method}")]
fn then_calls_contain(world: &ExtractionWorld, method: String) {
    assert!(
        world.has_call(&method),
        "expected called_methods to contain '{method}'"
    );
}

#[then("the called methods do not contain {method}")]
fn then_calls_not_contain(world: &ExtractionWorld, method: String) {
    assert!(
        !world.has_call(&method),
        "expected called_methods NOT to contain '{method}'"
    );
}

#[then("the accessed fields are empty")]
fn then_fields_empty(world: &ExtractionWorld) {
    assert!(world.fields_empty(), "expected accessed_fields to be empty");
}

#[then("the called methods are empty")]
fn then_calls_empty(world: &ExtractionWorld) {
    assert!(world.calls_empty(), "expected called_methods to be empty");
}

// Scenario indices must match their declaration order in
// `tests/features/method_extraction.feature`. Adding, removing, or
// reordering scenarios in the feature file requires updating the indices
// here.

#[scenario(path = "tests/features/method_extraction.feature", index = 0)]
fn scenario_field_access_recorded(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 1)]
fn scenario_method_call_recorded(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 2)]
fn scenario_macro_field_filtered(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 3)]
fn scenario_macro_call_filtered(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 4)]
fn scenario_all_expansion_empty(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 5)]
fn scenario_empty_builder(world: ExtractionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/method_extraction.feature", index = 6)]
fn scenario_multiple_accumulate(world: ExtractionWorld) {
    let _ = world;
}
