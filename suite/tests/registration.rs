//! Behaviour-driven tests for the suite registration wiring.
#![feature(rustc_private)]
#![cfg(feature = "dylint-driver")]

use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rustc_lint::LintStore;
use whitaker_common::experimental::{
    EXPERIMENTAL_LINTS as EXPERIMENTAL,
    ExperimentalSelection,
    resolve,
};
use whitaker_suite::{
    PassKind,
    register_suite_lints,
    suite_lint_decls,
    suite_lint_names,
    suite_lint_names_for_pass,
};

struct RegistrationWorld {
    store: RefCell<LintStore>,
    result: RefCell<Result<(), String>>,
    selection: RefCell<ExperimentalSelection>,
}

impl RegistrationWorld {
    fn new() -> Self {
        Self {
            store: RefCell::new(LintStore::new()),
            result: RefCell::new(Ok(())),
            selection: RefCell::new(ExperimentalSelection::none()),
        }
    }

    fn reset(&self) {
        *self.store.borrow_mut() = LintStore::new();
        *self.result.borrow_mut() = Ok(());
        *self.selection.borrow_mut() = ExperimentalSelection::none();
    }
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> RegistrationWorld { RegistrationWorld::new() }

#[given("an empty lint store")]
fn given_empty_store(world: &RegistrationWorld) { world.reset(); }

#[given("the suite lints are already registered")]
fn given_already_registered(world: &RegistrationWorld) {
    given_empty_store(world);
    register_suite_lints(&mut world.store.borrow_mut(), &world.selection.borrow());
}

#[given("the repository opts in to {name}")]
fn given_opts_in(world: &RegistrationWorld, name: String) {
    let selection = resolve(&[name.as_str()]).unwrap_or_else(|error| {
        panic!("the scenario must name a known experimental lint: {error}");
    });
    let merged = world.selection.take().union(&selection);
    *world.selection.borrow_mut() = merged;
}

#[given("every experimental lint is enabled")]
fn given_all_enabled(world: &RegistrationWorld) {
    *world.selection.borrow_mut() = ExperimentalSelection::all();
}

#[when("I register the suite lints")]
fn when_register_suite(world: &RegistrationWorld) {
    let registration = catch_unwind(AssertUnwindSafe(|| {
        register_suite_lints(&mut world.store.borrow_mut(), &world.selection.borrow());
    }))
    .map_err(|panic| {
        panic.downcast_ref::<&str>().map_or_else(
            || {
                panic.downcast_ref::<String>().map_or_else(
                    || "registration panicked with a non-string payload".to_owned(),
                    Clone::clone,
                )
            },
            |message| (*message).to_owned(),
        )
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

#[then("the early pass count is {count}")]
fn then_early_pass_count(world: &RegistrationWorld, count: usize) {
    assert_eq!(world.store.borrow().early_passes.len(), count);
}

/// Asserts the registered early pass reports the string-continuation lint.
///
/// This is the assertion the RFC asks for. A count alone passes when the wrong
/// pass has been registered, and unlike a late lint omitted from
/// `late_lint_methods!`, a missing or mistaken `register_early_pass` line is
/// not a compile error.
#[then("the early pass is the string continuation style pass")]
fn then_early_pass_is_string_continuation(world: &RegistrationWorld) {
    let store = world.store.borrow();
    let registered: Vec<String> = store
        .early_passes
        .iter()
        .flat_map(|factory| factory().get_lints())
        .map(rustc_lint::Lint::name_lower)
        .collect();

    assert_eq!(
        registered,
        vec![String::from("string_continuation_style")],
        "the registered early pass must declare exactly the string-continuation lint",
    );
}

/// Asserts every selected descriptor tagged `Early` has a pass registered.
///
/// The descriptor list and the `register_early_pass` call are separate sources
/// of truth. Tying the assertion to `suite_lint_names_for_pass` means adding a
/// second early lint without registering it fails here rather than silently
/// declaring a lint that never runs. Unselected early lints must have no pass.
#[then("every selected early lint descriptor has a registered early pass")]
fn then_every_early_descriptor_is_registered(world: &RegistrationWorld) {
    let store = world.store.borrow();
    let registered: Vec<String> = store
        .early_passes
        .iter()
        .flat_map(|factory| factory().get_lints())
        .map(rustc_lint::Lint::name_lower)
        .collect();

    let selection = world.selection.borrow();
    let expected: Vec<String> = suite_lint_names_for_pass(PassKind::Early)
        .filter(|name| !EXPERIMENTAL.contains(name) || selection.contains(name))
        .map(str::to_owned)
        .collect();

    assert_eq!(registered, expected);
}

/// Asserts the registered declarations include the macro-aware complexity lint.
///
/// A late-pass factory needs a real `TyCtxt` to construct the combined pass, so
/// unlike the early-pass assertion above this cannot invoke the factory and ask
/// it what it declares. Instead it ties three separately maintained views
/// together: the descriptor tagged `Late`, the static declaration list, and the
/// lints the store actually registered.
#[then("the late pass declares the macro-aware cognitive complexity lint")]
fn then_late_pass_declares_macro_aware(world: &RegistrationWorld) {
    const NAME: &str = "macro_aware_cognitive_complexity";

    let late: Vec<&str> = suite_lint_names_for_pass(PassKind::Late).collect();
    assert_eq!(
        late.iter().filter(|name| **name == NAME).count(),
        1,
        "the late phase must describe the macro-aware lint exactly once, got {late:?}",
    );

    let declared: Vec<String> = suite_lint_decls()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    assert_eq!(
        declared.iter().filter(|name| name.as_str() == NAME).count(),
        1,
        "the suite declarations must include the macro-aware lint exactly once, got {declared:?}",
    );

    let registered: Vec<String> = world
        .store
        .borrow()
        .get_lints()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    assert_eq!(
        registered
            .iter()
            .filter(|name| name.as_str() == NAME)
            .count(),
        1,
        "the lint store must register the macro-aware lint exactly once, got {registered:?}",
    );
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
    let expected: Vec<String> = suite_lint_names().map(str::to_owned).collect();

    assert_eq!(registered, expected);
}

#[then("the suite lint declarations align with the descriptors")]
fn then_decls_align() {
    let declared: Vec<String> = suite_lint_decls()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    let expected: Vec<String> = suite_lint_names().map(str::to_owned).collect();

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
fn scenario_registers_cleanly(world: RegistrationWorld) { let _ = world; }

#[scenario(path = "tests/features/suite_registration.feature", index = 1)]
fn scenario_registers_no_experimental_pass_without_opt_in(world: RegistrationWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/suite_registration_experimental.feature",
    index = 0
)]
fn scenario_registers_early_pass(world: RegistrationWorld) { let _ = world; }

#[scenario(
    path = "tests/features/suite_registration_late_experimental.feature",
    index = 0
)]
fn scenario_registers_late_experimental_lint(world: RegistrationWorld) { let _ = world; }

#[scenario(
    path = "tests/features/suite_registration_late_experimental.feature",
    index = 1
)]
fn scenario_registers_rstest_helper_lint(world: RegistrationWorld) { let _ = world; }

#[scenario(
    path = "tests/features/suite_registration_late_experimental.feature",
    index = 2
)]
fn scenario_registers_every_experimental_lint(world: RegistrationWorld) { let _ = world; }

#[scenario(path = "tests/features/suite_registration.feature", index = 2)]
fn scenario_double_registration(world: RegistrationWorld) { let _ = world; }

/// Verify that Dylint loads the suite through its exported `register_lints` entry point.
#[test]
fn exported_entry_point_registers_suite_lints() {
    whitaker_lint_core::run_ui_tests!("tests/ui")
        .expect("Dylint should load the suite and satisfy its registered lint expectation");
}
