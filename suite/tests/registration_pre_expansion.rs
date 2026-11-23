//! Registration tests for the suite's pre-expansion pass.
//!
//! Kept in their own file because `registration.rs` is already close to the
//! 400-line ceiling, and because a pre-expansion pass needs a different
//! assertion from a late or early one: it is registered into a third store
//! list, and its factory must be invoked to see which lints it declares.

#![feature(rustc_private)]
#![cfg(feature = "dylint-driver")]

use rustc_lint::LintStore;
use whitaker_common::experimental::{ExperimentalSelection, resolve};
use whitaker_suite::{
    PassKind,
    experimental_passes,
    register_suite_lints,
    suite_lint_names_for_pass,
};

/// The lint this file exists to check.
const NAME: &str = "missing_docs_in_test_functions";

/// A repository that has not opted in gets no pre-expansion pass at all.
///
/// The lint is still declared, so a leftover `#[allow]` naming it does not trip
/// `unknown_lints`, but nothing can run: an unselected experimental lint must be
/// inert rather than merely quiet.
#[test]
fn an_unselected_lint_is_declared_but_has_no_pass() {
    let mut store = LintStore::new();
    register_suite_lints(&mut store, &ExperimentalSelection::none());

    let declared: Vec<String> = store
        .get_lints()
        .iter()
        .map(rustc_lint::Lint::name_lower)
        .collect();
    assert_eq!(
        declared.iter().filter(|name| name.as_str() == NAME).count(),
        1,
        "the suite must declare the lint even when no repository opts in, got {declared:?}",
    );
    assert_eq!(
        store.pre_expansion_passes.len(),
        0,
        "an unselected experimental lint must register no pre-expansion pass"
    );
}

/// An opt-in registers exactly one pre-expansion pass, and that pass declares
/// exactly this lint.
#[test]
fn an_opted_in_lint_registers_exactly_one_matching_pass() {
    let mut store = LintStore::new();
    let selection = resolve(&[NAME]).expect("the lint is a known experimental lint");
    register_suite_lints(&mut store, &selection);

    assert_eq!(
        store.pre_expansion_passes.len(),
        1,
        "the opt-in must register exactly one pre-expansion pass"
    );
    let registered: Vec<String> = store
        .pre_expansion_passes
        .iter()
        .flat_map(|factory| factory().get_lints())
        .map(rustc_lint::Lint::name_lower)
        .collect();
    assert_eq!(
        registered,
        [String::from(NAME)],
        "the registered pre-expansion pass must declare exactly this lint"
    );
}

/// Every descriptor tagged `PreExpansion` has a table entry, and vice versa.
///
/// A descriptor without a table entry is a lint the suite promises to run and
/// silently never does; a table entry without a descriptor is a pass whose
/// phase the registration tests cannot check.
#[test]
fn pre_expansion_descriptors_and_table_entries_agree() {
    let mut described: Vec<&str> = suite_lint_names_for_pass(PassKind::PreExpansion).collect();
    let mut tabled: Vec<&str> = experimental_passes()
        .iter()
        .filter(|pass| pass.kind == PassKind::PreExpansion)
        .map(|pass| pass.name)
        .collect();
    described.sort_unstable();
    tabled.sort_unstable();
    assert_eq!(
        described, tabled,
        "the pre-expansion descriptors and pass table differ"
    );
    assert_eq!(
        described,
        [NAME],
        "this item ships exactly one pre-expansion lint"
    );
}
