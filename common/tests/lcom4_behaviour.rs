//! Behaviour-driven coverage for LCOM4 cohesion analysis.

use common::lcom4::{MethodInfo, cohesion_components};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
struct LcomWorld {
    methods: RefCell<Vec<MethodInfo>>,
    result: Cell<Option<usize>>,
}

impl LcomWorld {
    fn push_method(&self, method: MethodInfo) {
        self.methods.borrow_mut().push(method);
    }

    fn compute(&self) {
        let methods = self.methods.borrow();
        self.result.set(Some(cohesion_components(&methods)));
    }

    #[expect(
        clippy::expect_used,
        reason = "fixture configuration is required for this behaviour test"
    )]
    fn result(&self) -> usize {
        self.result
            .get()
            .expect("LCOM4 result must be computed before assertion")
    }
}

#[fixture]
fn world() -> LcomWorld {
    LcomWorld::default()
}

/// Parses a comma-separated list of field names into a `BTreeSet`.
///
/// Whitespace around each name is trimmed and empty segments are skipped.
fn parse_field_set(fields: &str) -> BTreeSet<String> {
    fields
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

#[given("a method called {name} accessing fields {fields}")]
fn given_method_with_fields(world: &LcomWorld, name: String, fields: String) {
    world.push_method(MethodInfo::new(
        name,
        parse_field_set(&fields),
        BTreeSet::new(),
    ));
}

#[given("a method called {name} accessing no fields")]
fn given_method_no_fields(world: &LcomWorld, name: String) {
    world.push_method(MethodInfo::new(name, BTreeSet::new(), BTreeSet::new()));
}

#[given("a method called {name} accessing no fields calling {callee}")]
fn given_method_no_fields_calling(world: &LcomWorld, name: String, callee: String) {
    world.push_method(MethodInfo::new(
        name,
        BTreeSet::new(),
        BTreeSet::from([callee]),
    ));
}

#[when("I compute LCOM4")]
fn when_compute(world: &LcomWorld) {
    world.compute();
}

#[then("the component count is {count}")]
fn then_component_count(world: &LcomWorld, count: usize) {
    assert_eq!(world.result(), count);
}

#[scenario(path = "tests/features/lcom4.feature", index = 0)]
fn scenario_single_method(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 1)]
fn scenario_shared_field(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 2)]
fn scenario_direct_call(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 3)]
fn scenario_disjoint_methods(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 4)]
fn scenario_transitive_sharing(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 5)]
fn scenario_empty_type(world: LcomWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lcom4.feature", index = 6)]
fn scenario_isolated_methods(world: LcomWorld) {
    let _ = world;
}
