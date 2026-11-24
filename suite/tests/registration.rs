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
    experimental_passes,
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

/// Add the named experimental lint to the repository's opt-in.
#[given("the repository opts in to {name}")]
fn given_opts_in(world: &RegistrationWorld, name: String) {
    let selection = resolve(&[name.as_str()]).unwrap_or_else(|error| {
        panic!("the scenario must name a known experimental lint: {error}");
    });
    let merged = world.selection.take().union(&selection);
    *world.selection.borrow_mut() = merged;
}

/// Enable every experimental lint, as a local `--experimental` build does.
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

/// Asserts the store declares the macro-aware complexity lint exactly once.
///
/// This is about declarations, not passes: a declared lint may have no pass.
/// A late-pass factory needs a real `TyCtxt`, so which late pass was wired is
/// pinned by `experimental_pass_entries_register_the_pass_their_descriptor_names`
/// instead. This step ties three separately maintained views together: the
/// descriptor tagged `Late`, the static declaration list, and the lints the
/// store actually registered.
#[then("the macro-aware cognitive complexity lint is declared")]
fn then_macro_aware_is_declared(world: &RegistrationWorld) {
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

/// Without an opt-in no experimental pass is registered.
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

/// Opting in to the rstest helper lint registers its own late pass.
#[scenario(
    path = "tests/features/suite_registration_late_experimental.feature",
    index = 1
)]
fn scenario_registers_rstest_helper_lint(world: RegistrationWorld) { let _ = world; }

/// Enabling every experimental lint registers every experimental pass.
#[scenario(
    path = "tests/features/suite_registration_late_experimental.feature",
    index = 2
)]
fn scenario_registers_every_experimental_lint(world: RegistrationWorld) { let _ = world; }

#[scenario(path = "tests/features/suite_registration.feature", index = 2)]
fn scenario_double_registration(world: RegistrationWorld) { let _ = world; }

/// The pass table covers exactly the shared experimental lints, so a lint
/// added to one list and not the other fails here.
#[test]
fn experimental_pass_table_covers_exactly_the_shared_experimental_lints() {
    let mut table: Vec<&str> = experimental_passes().iter().map(|pass| pass.name).collect();
    let mut shared = EXPERIMENTAL.to_vec();
    table.sort_unstable();
    shared.sort_unstable();
    assert_eq!(
        table, shared,
        "pass table and shared experimental list differ"
    );
}

/// Each entry declares exactly the lint it is named for. A late pass cannot be
/// inspected once registered, because its factory needs a `TyCtxt`; but each
/// entry is built from one pass type, so the lints that type declares identify
/// which pass the entry registers. An entry built from another lint's pass type
/// declares that lint's name instead, and this fails.
#[test]
fn experimental_pass_entries_declare_the_lint_they_are_named_for() {
    for pass in experimental_passes() {
        assert_eq!(
            (pass.declared_lints)(),
            [pass.name],
            "the pass registered for {} declares a different lint",
            pass.name
        );
    }
}

/// Each table entry registers one pass in the phase its descriptor names, and
/// that pass is the only one it adds.
#[test]
fn experimental_pass_entries_register_one_pass_in_their_descriptor_phase() {
    for pass in experimental_passes() {
        let mut store = LintStore::new();
        (pass.register)(&mut store);

        let (late, early) = (store.late_passes.len(), store.early_passes.len());
        let (expected_late, expected_early) = match pass.kind {
            PassKind::Late => (1, 0),
            PassKind::Early => (0, 1),
        };
        assert_eq!(
            (late, early),
            (expected_late, expected_early),
            "{} must register exactly one {:?} pass",
            pass.name,
            pass.kind
        );
        assert!(
            suite_lint_names_for_pass(pass.kind).any(|name| name == pass.name),
            "{} is tagged {:?} here but not in the suite descriptors",
            pass.name,
            pass.kind
        );
    }
}

/// Enabling every experimental lint registers the fused pass plus exactly one
/// pass per experimental lint, none twice.
#[test]
fn enabling_every_experimental_lint_registers_each_pass_once() {
    let mut store = LintStore::new();
    let selection = ExperimentalSelection::all();
    register_suite_lints(&mut store, &selection);
    let passes = experimental_passes();
    let count_of = |kind| passes.iter().filter(|pass| pass.kind == kind).count();
    assert_eq!(
        store.late_passes.len(),
        1 + count_of(PassKind::Late),
        "the fused pass plus one per experimental late lint"
    );
    assert_eq!(
        store.early_passes.len(),
        count_of(PassKind::Early),
        "one early pass per experimental early lint"
    );
}

/// Verify that Dylint loads the suite through its exported `register_lints` entry point.
#[test]
fn exported_entry_point_registers_suite_lints() {
    whitaker_lint_core::run_ui_tests!("tests/ui")
        .expect("Dylint should load the suite and satisfy its registered lint expectation");
}
