//! Tests the two macro-name predicates stay deliberately distinct.
//!
//! The RFC's format-macro set and the set the lint is allowed to *spell* are
//! not the same set, and the difference is entirely `concat`. The shadowing
//! check must cover `concat` because the suggested replacement names it; the
//! format-position check must not, because a format string reached through a
//! user `concat!` is not in format position and has no captured arguments to
//! lose. A single shared list would silently grant one of those checks the
//! other's coverage.

use super::{FORMAT_MACRO_NAMES, is_builtin_macro_name, is_format_macro_name};

#[test]
fn every_rfc_format_macro_name_is_accepted() {
    for name in FORMAT_MACRO_NAMES {
        assert!(
            is_format_macro_name(name),
            "{name} is in the RFC's set and must be accepted"
        );
    }
}

#[test]
fn the_format_set_covers_exactly_the_rfc_names() {
    assert_eq!(FORMAT_MACRO_NAMES.len(), 18);
    for required in [
        "format_args",
        "format",
        "print",
        "println",
        "eprint",
        "eprintln",
        "write",
        "writeln",
        "panic",
        "todo",
        "unimplemented",
        "unreachable",
        "assert",
        "debug_assert",
        "assert_eq",
        "assert_ne",
        "debug_assert_eq",
        "debug_assert_ne",
    ] {
        assert!(
            is_format_macro_name(required),
            "{required} is required by RFC-0002 §Macro position"
        );
    }
}

#[test]
fn concat_is_not_a_format_macro_name() {
    assert!(!is_format_macro_name("concat"));
}

#[test]
fn concat_is_a_name_the_lint_must_refuse_to_shadow() {
    assert!(is_builtin_macro_name("concat"));
}

#[test]
fn the_builtin_set_is_the_format_set_plus_concat() {
    // Checking membership against the composed predicate rather than a second
    // literal list keeps this test from restating the set it is meant to guard.
    for name in FORMAT_MACRO_NAMES {
        assert!(is_builtin_macro_name(name), "{name} must still shadow");
    }
    assert!(is_builtin_macro_name("concat"));
    assert!(!is_builtin_macro_name("vec"));
    assert!(!is_builtin_macro_name("format_args_nl"));
}

#[test]
fn names_outside_the_sets_are_rejected_by_both_predicates() {
    for name in ["log", "info", "tracing", "dbg", "concat_bytes", ""] {
        assert!(!is_format_macro_name(name), "{name} is not a format macro");
        assert!(
            !is_builtin_macro_name(name),
            "{name} is not a name the lint spells"
        );
    }
}
