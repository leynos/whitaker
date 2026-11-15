//! Focused registry assertions for the experimental macro-aware lint.
//!
//! The registration scenarios exercise a real `LintStore`; this target pins the
//! two static registry views instead. The lint is compiled into every suite,
//! so both views must list it exactly once whether or not a repository opts in.
#![feature(rustc_private)]
#![cfg(feature = "dylint-driver")]

use whitaker_suite::{PassKind, suite_lint_decls, suite_lint_names, suite_lint_names_for_pass};

/// The lint name under test.
const NAME: &str = "macro_aware_cognitive_complexity";

/// Asserts the suite wires the lint exactly once, as an experimental late pass.
#[test]
fn suite_declares_the_lint_once_in_the_late_pass() {
    let names: Vec<&str> = suite_lint_names().collect();
    assert_eq!(
        names.iter().filter(|name| **name == NAME).count(),
        1,
        "the suite must list the macro-aware lint exactly once, got {names:?}",
    );

    let late: Vec<&str> = suite_lint_names_for_pass(PassKind::Late).collect();
    assert_eq!(
        late.iter().filter(|name| **name == NAME).count(),
        1,
        "the macro-aware lint must be a late pass, got {late:?}",
    );

    let early: Vec<&str> = suite_lint_names_for_pass(PassKind::Early).collect();
    assert!(
        !early.contains(&NAME),
        "the macro-aware lint must not also appear as an early pass, got {early:?}",
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
}

/// Asserts the descriptors mark exactly the shared experimental list.
#[test]
fn descriptors_mark_the_shared_experimental_lints() {
    let marked: Vec<&str> = whitaker_suite::SUITE_LINTS
        .iter()
        .filter(|descriptor| descriptor.is_experimental())
        .map(|descriptor| descriptor.name)
        .collect();
    let mut shared = whitaker_common::experimental::EXPERIMENTAL_LINTS.to_vec();
    let mut found = marked;
    shared.sort_unstable();
    found.sort_unstable();
    assert_eq!(
        found, shared,
        "the suite and whitaker-common disagree on the experimental set"
    );
}
