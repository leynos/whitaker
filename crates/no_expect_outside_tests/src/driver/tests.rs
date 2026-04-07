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
#[case::core_prelude_test(&["core", "prelude", "v1", "test"])]
#[case::std_prelude_test(&["std", "prelude", "rust_2024", "test"])]
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
#[case::wrong_prelude_root(&["foo", "prelude", "v1", "test"])]
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

fn assert_has_test_attribute(attr_segments: &[&[&str]], expected: bool) {
    create_default_session_globals_then(|| {
        let attrs: Vec<hir::Attribute> = attr_segments
            .iter()
            .map(|segments| hir_attribute_from_segments(segments))
            .collect();
        assert_eq!(has_test_attribute(&attrs), expected);
    });
}

#[rstest]
#[case::inline_and_test(0)]
#[case::rstest_only(1)]
#[case::tokio_test(2)]
#[case::core_prelude_test(3)]
fn has_test_attribute_detects_test_attributes(#[case] case_index: usize) {
    let test_cases: &[&[&[&str]]] = &[
        &[&["inline"], &["test"]],
        &[&["rstest"]],
        &[&["tokio", "test"]],
        &[&["core", "prelude", "v1", "test"]],
    ];
    assert_has_test_attribute(test_cases[case_index], true);
}

#[test]
fn has_test_attribute_returns_false_for_empty_array() {
    let attrs: [hir::Attribute; 0] = [];
    assert!(!has_test_attribute(&attrs));
}

#[test]
fn has_test_attribute_returns_false_for_non_test_attributes() {
    assert_has_test_attribute(&[&["inline"], &["derive"], &["tokio", "main"]], false);
}

#[test]
fn has_test_attribute_handles_parsed_attributes() {
    let attrs = [parsed_must_use_attribute()];
    assert!(!has_test_attribute(&attrs));
}

// -------------------------------------------------------------------------
// Tests for has_test_module_name
// -------------------------------------------------------------------------

#[rstest]
#[case::exact_test("test", true)]
#[case::exact_tests("tests", true)]
#[case::prefix_test_helpers("test_helpers", true)]
#[case::prefix_tests_util("tests_util", true)]
#[case::suffix_service_tests("service_tests", true)]
#[case::suffix_api_test("api_test", true)]
#[case::suffix_integration_tests("integration_tests", true)]
#[case::suffix_unit_test("unit_test", true)]
#[case::plain_service("my_service", false)]
#[case::testing("testing", false)]
#[case::attest("attest", false)]
#[case::contest("contest", false)]
#[case::test_embedded_not_suffix("test_like_utils", true)]
#[case::empty_string("", false)]
fn has_test_module_name_matches_test_conventions(#[case] name: &str, #[case] expected: bool) {
    assert_eq!(has_test_module_name(name), expected, "name = {name:?}");
}

// -------------------------------------------------------------------------
// Coverage notes for is_test_named_module, extract_function_item, and the
// is_likely_test_function fallback
//
// These helpers require full HIR context (hir::Node, hir::Item) which cannot
// be constructed in unit tests without mocking the entire compiler
// infrastructure. The fallback logic (is_likely_test_function) also requires
// the --test harness flag which isn't set during UI test compilation.
//
// `has_test_module_name` is tested directly above since it is a pure
// string predicate that does not depend on HIR.
//
// Behavioural coverage is achieved through:
//
// 1. UI tests for attribute detection (is_test_attribute, has_test_attribute):
//    - pass_expect_in_test.rs, pass_expect_in_rstest.rs, pass_expect_in_tokio_test.rs
//    - These verify that test attributes are recognized without the fallback
//
// 2. UI tests for cfg(test) module detection:
//    - pass_expect_in_test_module.rs, pass_expect_in_tests_module.rs
//    - These verify #[cfg(test)] mod test/tests detection
//
// 3. UI tests for `#[path]`-loaded modules with non-standard names:
//    - pass_expect_in_path_module_tokio_test.rs verifies attribute detection
//      inside `#[path]`-loaded modules whose names match `has_test_module_name`
//    - pass_expect_in_cfg_test_named_module.rs verifies `#[cfg(test)]`
//      detection for non-standard module names with `#[tokio::test]`
//
// 4. Example-based regression coverage for the `rustc --test` harness path:
//    - `pass_expect_in_tokio_test_harness` compiles a real `#[tokio::test]`
//      example target under `--test`, placing `.expect(...)` calls inside
//      nested closure and async-block bodies so the parent walk and sibling
//      const descriptor fallback are both exercised.
//    - `pass_expect_in_path_module_harness` compiles a `#[tokio::test]` in
//      a `#[path]`-loaded module with a non-standard name under `--test`,
//      exercising the extended `has_test_module_name` fallback.
//
// 5. Real-world validation: The lint is used on this repository's own
//    integration tests (compiled with --test), validating the fallback works
//    correctly for tests/ directory detection and module-name heuristics.
//
// The individual helper functions have straightforward pattern matching:
// - is_test_named_module: delegates to has_test_module_name for name matching
// - has_test_module_name: matches exact and affixed test module name patterns
// - extract_function_item: matches `hir::Node::Item` values whose kind is `Fn`
// -------------------------------------------------------------------------
