#![feature(rustc_private)]
#![cfg(feature = "dylint-driver")]
//! Behaviour-driven tests for the suite registration wiring.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rustc_lint::LintStore;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use whitaker_suite::{register_suite_lints, suite_lint_decls, suite_lint_names};

struct RegistrationWorld {
    store: RefCell<LintStore>,
    result: RefCell<Result<(), String>>,
}

impl RegistrationWorld {
    fn new() -> Self {
        Self {
            store: RefCell::new(LintStore::new()),
            result: RefCell::new(Ok(())),
        }
    }

    fn reset(&self) {
        *self.store.borrow_mut() = LintStore::new();
        *self.result.borrow_mut() = Ok(());
    }
}

#[fixture]
fn world() -> RegistrationWorld {
    RegistrationWorld::new()
}

#[given("an empty lint store")]
fn given_empty_store(world: &RegistrationWorld) {
    world.reset();
}

#[given("the suite lints are already registered")]
fn given_already_registered(world: &RegistrationWorld) {
    given_empty_store(world);
    register_suite_lints(&mut world.store.borrow_mut());
}

#[when("I register the suite lints")]
fn when_register_suite(world: &RegistrationWorld) {
    let registration = catch_unwind(AssertUnwindSafe(|| {
        register_suite_lints(&mut world.store.borrow_mut());
    }))
    .map(|_| ())
    .map_err(|panic| {
        if let Some(message) = panic.downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else {
            "registration panicked with a non-string payload".to_string()
        }
    });

    *world.result.borrow_mut() = registration;
}

#[then("the store has the suite lints registered")]
fn then_registered_suite_lints(world: &RegistrationWorld) {
    let expected = suite_lint_names().count();
    assert_eq!(world.store.borrow().get_lints().len(), expected);
}

#[then("the late pass count is {count}")]
fn then_late_pass_count(world: &RegistrationWorld, count: usize) {
    assert_eq!(world.store.borrow().late_passes.len(), count);
}

#[then("the lint names mirror the suite descriptors")]
fn then_names_match(world: &RegistrationWorld) {
    let registered: Vec<String> = world
        .store
        .borrow()
        .get_lints()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    let expected: Vec<String> = suite_lint_names().map(str::to_string).collect();

    assert_eq!(registered, expected);
}

#[then("the suite lint declarations align with the descriptors")]
fn then_decls_align() {
    let declared: Vec<String> = suite_lint_decls()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    let expected: Vec<String> = suite_lint_names().map(str::to_string).collect();

    assert_eq!(declared, expected);
}

#[then("registration fails with a duplicate lint error")]
fn then_registration_fails(world: &RegistrationWorld) {
    assert!(world.result.borrow().is_err(), "registration should fail");
}

#[then("registration succeeds")]
fn then_registration_succeeds(world: &RegistrationWorld) {
    assert!(world.result.borrow().is_ok());
}

#[scenario(path = "tests/features/suite_registration.feature", index = 0)]
fn scenario_registers_cleanly(world: RegistrationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/suite_registration.feature", index = 1)]
fn scenario_double_registration(world: RegistrationWorld) {
    let _ = world;
}
