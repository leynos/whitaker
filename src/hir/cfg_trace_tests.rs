//! Tests for the `cfg(test)` recognition shared by the expect and unwrap
//! lints.
//!
//! These build `CfgEntry` values directly rather than through a
//! compilation. The recognizer is a pure function over that tree, and the
//! shapes that matter, negation and disjunction, are precisely the ones a
//! UI fixture cannot express without a separate crate per case.

use super::gates_on_test;
use rustc_data_structures::thin_vec::thin_vec;
use rustc_hir::attrs::CfgEntry;
use rustc_span::{DUMMY_SP, Symbol, create_default_session_globals_then};

fn name(value: &str) -> CfgEntry {
    CfgEntry::NameValue {
        name: Symbol::intern(value),
        value: None,
        span: DUMMY_SP,
    }
}

#[test]
fn a_bare_test_predicate_gates_on_test() {
    create_default_session_globals_then(|| {
        assert!(gates_on_test(&name("test")));
    });
}

#[test]
fn another_predicate_does_not() {
    create_default_session_globals_then(|| {
        assert!(!gates_on_test(&name("unix")));
    });
}

#[test]
fn negation_inverts_the_answer() {
    // `not(test)` leaves the item present only outside a test build, so
    // reading it as test context would exempt production code.
    create_default_session_globals_then(|| {
        let negated = CfgEntry::Not(Box::new(name("test")), DUMMY_SP);
        assert!(!gates_on_test(&negated));
    });
}

#[test]
fn double_negation_restores_it() {
    create_default_session_globals_then(|| {
        let inner = CfgEntry::Not(Box::new(name("test")), DUMMY_SP);
        let outer = CfgEntry::Not(Box::new(inner), DUMMY_SP);
        assert!(gates_on_test(&outer));
    });
}

#[test]
fn a_group_without_test_does_not_count() {
    create_default_session_globals_then(|| {
        let entry = CfgEntry::All(thin_vec![name("unix"), name("feature")], DUMMY_SP);
        assert!(!gates_on_test(&entry));
    });
}

#[test]
fn negation_carries_into_a_group() {
    // `not(any(test, unix))` excludes test builds, so nothing inside it is
    // test context.
    create_default_session_globals_then(|| {
        let group = CfgEntry::Any(thin_vec![name("test"), name("unix")], DUMMY_SP);
        let negated = CfgEntry::Not(Box::new(group), DUMMY_SP);
        assert!(!gates_on_test(&negated));
    });
}

#[test]
fn literals_and_versions_are_not_test_gates() {
    create_default_session_globals_then(|| {
        assert!(!gates_on_test(&CfgEntry::Bool(true, DUMMY_SP)));
        assert!(!gates_on_test(&CfgEntry::Version(None, DUMMY_SP)));
    });
}

#[test]
fn a_disjunction_with_a_production_predicate_is_not_test_only() {
    // The case that matters most. `any(test, feature = "x")` keeps the item in
    // a production build that enables `x`, so treating it as test-only would
    // exempt production code from the lint. Mentioning `test` is not the
    // question; being unsatisfiable without `test` is.
    create_default_session_globals_then(|| {
        let entry = CfgEntry::Any(thin_vec![name("test"), name("feature")], DUMMY_SP);
        assert!(!gates_on_test(&entry));
    });
}

#[test]
fn a_disjunction_of_test_predicates_is_test_only() {
    // Every arm requires `test`, so nothing satisfies it in a production
    // build. The rule is about the predicate, not about the operator.
    create_default_session_globals_then(|| {
        let entry = CfgEntry::Any(thin_vec![name("test"), name("test")], DUMMY_SP);
        assert!(gates_on_test(&entry));
    });
}

#[test]
fn a_conjunction_needs_only_one_test_arm() {
    // `all(test, unix)` cannot hold without `test`, whatever `unix` does.
    create_default_session_globals_then(|| {
        let entry = CfgEntry::All(thin_vec![name("test"), name("unix")], DUMMY_SP);
        assert!(gates_on_test(&entry));
    });
}

#[test]
fn a_negated_conjunction_is_not_test_only() {
    // `not(all(test, unix))` holds whenever `unix` is off, test build or not.
    create_default_session_globals_then(|| {
        let group = CfgEntry::All(thin_vec![name("test"), name("unix")], DUMMY_SP);
        assert!(!gates_on_test(&CfgEntry::Not(Box::new(group), DUMMY_SP)));
    });
}
