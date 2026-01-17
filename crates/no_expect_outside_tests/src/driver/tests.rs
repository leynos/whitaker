//! Unit tests for test attribute detection helpers in the driver module.

use super::*;
use rstest::rstest;
use rustc_ast::AttrStyle;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_span::symbol::Ident;
use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};

// -------------------------------------------------------------------------
// Test fixtures for HIR attributes
// -------------------------------------------------------------------------

fn hir_attribute_from_segments(segments: &[&str]) -> hir::Attribute {
    let path_segments = segments
        .iter()
        .map(|segment| Ident::from_str(segment))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let attr_item = hir::AttrItem {
        path: hir::AttrPath {
            segments: path_segments,
            span: DUMMY_SP,
        },
        args: hir::AttrArgs::Empty,
        id: hir::HashIgnoredAttrId {
            attr_id: AttrId::from_u32(0),
        },
        style: AttrStyle::Outer,
        span: DUMMY_SP,
    };

    hir::Attribute::Unparsed(Box::new(attr_item))
}

fn parsed_must_use_attribute() -> hir::Attribute {
    hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    })
}

// -------------------------------------------------------------------------
// Tests for is_test_attribute
// -------------------------------------------------------------------------

#[rstest]
#[case::builtin_test(&["test"])]
#[case::rstest_single(&["rstest"])]
#[case::case_single(&["case"])]
#[case::tokio_test(&["tokio", "test"])]
#[case::async_std_test(&["async_std", "test"])]
#[case::gpui_test(&["gpui", "test"])]
#[case::rstest_rstest(&["rstest", "rstest"])]
#[case::rstest_case(&["rstest", "case"])]
fn is_test_attribute_accepts_test_patterns(#[case] segments: &[&str]) {
    create_default_session_globals_then(|| {
        let attr = hir_attribute_from_segments(segments);
        assert!(is_test_attribute(&attr));
    });
}

#[rstest]
#[case::tokio_main(&["tokio", "main"])]
#[case::rstest_fixture(&["rstest", "fixture"])]
#[case::inline(&["inline"])]
#[case::derive(&["derive"])]
#[case::allow(&["allow"])]
#[case::cfg(&["cfg"])]
#[case::three_segments(&["foo", "bar", "test"])]
fn is_test_attribute_rejects_non_test_attributes(#[case] segments: &[&str]) {
    create_default_session_globals_then(|| {
        let attr = hir_attribute_from_segments(segments);
        assert!(!is_test_attribute(&attr));
    });
}

#[test]
fn is_test_attribute_returns_false_for_parsed_attributes() {
    let attr = parsed_must_use_attribute();
    assert!(!is_test_attribute(&attr));
}

// -------------------------------------------------------------------------
// Tests for has_test_attribute
// -------------------------------------------------------------------------

#[test]
fn has_test_attribute_returns_true_when_test_present() {
    create_default_session_globals_then(|| {
        let attrs = [
            hir_attribute_from_segments(&["inline"]),
            hir_attribute_from_segments(&["test"]),
        ];
        assert!(has_test_attribute(&attrs));
    });
}

#[test]
fn has_test_attribute_returns_true_for_rstest() {
    create_default_session_globals_then(|| {
        let attrs = [hir_attribute_from_segments(&["rstest"])];
        assert!(has_test_attribute(&attrs));
    });
}

#[test]
fn has_test_attribute_returns_true_for_tokio_test() {
    create_default_session_globals_then(|| {
        let attrs = [hir_attribute_from_segments(&["tokio", "test"])];
        assert!(has_test_attribute(&attrs));
    });
}

#[test]
fn has_test_attribute_returns_false_for_empty_array() {
    let attrs: [hir::Attribute; 0] = [];
    assert!(!has_test_attribute(&attrs));
}

#[test]
fn has_test_attribute_returns_false_for_non_test_attributes() {
    create_default_session_globals_then(|| {
        let attrs = [
            hir_attribute_from_segments(&["inline"]),
            hir_attribute_from_segments(&["derive"]),
            hir_attribute_from_segments(&["tokio", "main"]),
        ];
        assert!(!has_test_attribute(&attrs));
    });
}

#[test]
fn has_test_attribute_handles_parsed_attributes() {
    let attrs = [parsed_must_use_attribute()];
    assert!(!has_test_attribute(&attrs));
}

// -------------------------------------------------------------------------
// Coverage notes for is_test_named_module, extract_function_item, and
// the is_likely_test_function fallback
//
// These helpers require full HIR context (hir::Node, hir::Item) which cannot
// be constructed in unit tests without mocking the entire compiler
// infrastructure. The fallback logic (is_likely_test_function) also requires
// the --test harness flag which isn't set during UI test compilation.
//
// Behavioural coverage is achieved through:
//
// 1. UI tests for attribute detection (is_test_attribute, has_test_attribute):
//    - pass_expect_in_test.rs, pass_expect_in_rstest.rs, pass_expect_in_tokio_test.rs
//    - These verify that test attributes are recognised without the fallback
//
// 2. UI tests for cfg(test) module detection:
//    - pass_expect_in_test_module.rs, pass_expect_in_tests_module.rs
//    - These verify #[cfg(test)] mod test/tests detection
//
// 3. Real-world validation: The lint is used on this repository's own
//    integration tests (compiled with --test), validating the fallback works
//    correctly for tests/ directory detection and module-name heuristics.
//
// The individual helper functions have straightforward pattern matching:
// - is_test_named_module: matches!(name, "test" | "tests")
// - extract_function_item: matches!(item.kind, ItemKind::Fn { .. })
// -------------------------------------------------------------------------
