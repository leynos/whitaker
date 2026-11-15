//! Focused registry assertions for the experimental macro-aware lint.
//!
//! The registration scenarios exercise a real `LintStore`; this target pins the
//! two static registry views instead. Both feature states are covered by a
//! single `#[cfg]`-conditional test so running the target under either feature
//! set performs a real assertion rather than an empty pass.
#![feature(rustc_private)]
#![cfg(feature = "dylint-driver")]

#[cfg(feature = "experimental-macro-aware-cognitive-complexity")]
use whitaker_suite::{PassKind, suite_lint_names_for_pass};
use whitaker_suite::{suite_lint_decls, suite_lint_names};

/// The lint name under test.
const NAME: &str = "macro_aware_cognitive_complexity";

/// Asserts the experimental build wires the lint exactly once, as a late pass.
#[cfg(feature = "experimental-macro-aware-cognitive-complexity")]
#[test]
fn experimental_build_declares_the_lint_once_in_the_late_pass() {
    let names: Vec<&str> = suite_lint_names().collect();
    assert_eq!(
        names.iter().filter(|name| **name == NAME).count(),
        1,
        "the experimental suite must list the macro-aware lint exactly once, got {names:?}",
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

/// Asserts the default build exposes neither the descriptor nor the declaration.
#[cfg(not(feature = "experimental-macro-aware-cognitive-complexity"))]
#[test]
fn default_build_omits_the_lint_from_names_and_declarations() {
    let names: Vec<&str> = suite_lint_names().collect();
    assert!(
        !names.contains(&NAME),
        "the default suite must not list the macro-aware lint, got {names:?}",
    );

    let declared: Vec<String> = suite_lint_decls()
        .iter()
        .map(|lint| lint.name_lower())
        .collect();
    assert!(
        !declared.iter().any(|name| name.as_str() == NAME),
        "the default suite must not declare the macro-aware lint, got {declared:?}",
    );
}
