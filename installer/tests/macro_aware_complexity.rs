//! Behaviour tests for delivering the experimental macro-aware lint.
//!
//! The scenarios cover both sides of the `--experimental` opt-in through plain
//! crate resolution; no compiler is invoked. The world is local because the
//! shared resolution world in `behaviour_core.rs` already serves its own
//! scenarios and extracting it would change that target's structure for no
//! behavioural gain.

use std::cell::{Cell, RefCell};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_installer::{
    crate_name::CrateName,
    resolution::{CrateResolutionOptions, resolve_crates},
};

/// The lint whose delivery is under test.
const LINT_NAME: &str = "macro_aware_cognitive_complexity";

/// Feature flags and resolved crates for one scenario.
#[derive(Default)]
struct MacroAwareWorld {
    /// Whether the request opts into experimental lints.
    experimental: Cell<bool>,
    /// Crates returned by the last resolution.
    resolved: RefCell<Vec<CrateName>>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn macro_aware_world() -> MacroAwareWorld { MacroAwareWorld::default() }

/// Record a request that does not opt into experimental lints.
#[given("an installation request without experimental lints")]
fn given_without_experimental(macro_aware_world: &MacroAwareWorld) {
    macro_aware_world.experimental.set(false);
}

/// Record a request that opts into experimental lints.
#[given("an installation request enabling experimental lints")]
fn given_with_experimental(macro_aware_world: &MacroAwareWorld) {
    macro_aware_world.experimental.set(true);
}

/// Resolve every individual lint crate for the recorded request.
///
/// Individual-lint mode is always selected so that "absent" reflects the
/// experimental opt-in rather than the default suite-only mode, which never
/// resolves individual crates.
#[when("the installer resolves lint crates")]
fn when_resolve_crates(macro_aware_world: &MacroAwareWorld) {
    let options = CrateResolutionOptions {
        individual_lints: true,
        experimental: macro_aware_world.experimental.get(),
    };
    macro_aware_world
        .resolved
        .replace(resolve_crates(&[], &options));
}

/// Assert the resolved set does not contain the macro-aware lint.
#[then("macro_aware_cognitive_complexity is absent")]
fn then_lint_absent(macro_aware_world: &MacroAwareWorld) {
    let resolved = macro_aware_world.resolved.borrow();
    assert!(
        !resolved.iter().any(|name| name.as_str() == LINT_NAME),
        "the stable request must not resolve {LINT_NAME}, got {resolved:?}"
    );
}

/// Assert the resolved set contains the macro-aware lint exactly once.
#[then("macro_aware_cognitive_complexity is selected exactly once")]
fn then_lint_selected_once(macro_aware_world: &MacroAwareWorld) {
    let resolved = macro_aware_world.resolved.borrow();
    let count = resolved
        .iter()
        .filter(|name| name.as_str() == LINT_NAME)
        .count();
    assert_eq!(
        count, 1,
        "the experimental request must resolve {LINT_NAME} once, got {resolved:?}"
    );
}

/// Run the stable-installation scenario.
#[scenario(path = "tests/features/macro_aware_complexity.feature", index = 0)]
fn scenario_stable_omits_lint(macro_aware_world: MacroAwareWorld) { let _ = macro_aware_world; }

/// Run the experimental-installation scenario.
#[scenario(path = "tests/features/macro_aware_complexity.feature", index = 1)]
fn scenario_experimental_includes_lint(macro_aware_world: MacroAwareWorld) {
    let _ = macro_aware_world;
}
