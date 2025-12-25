//! Unit tests for context conversion and attribute detection.
//!
//! Verifies HIR attribute conversion to `common::Attribute` and `cfg(test)`
//! detection for both parsed and unparsed attribute variants.

use super::{convert_attribute, is_cfg_test_attribute, meta_contains_test_cfg};
use common::AttributeKind;
use rstest::rstest;
use rustc_ast::ast::{MetaItem, MetaItemInner, MetaItemKind, Path, PathSegment, Safety};
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_span::symbol::Ident;
use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};

/// Type-safe wrapper for AST path segments.
#[derive(Debug, Clone, Copy)]
struct PathSegments(&'static [&'static str]);

impl PathSegments {
    const fn new(segments: &'static [&'static str]) -> Self {
        Self(segments)
    }

    fn as_slice(&self) -> &[&str] {
        self.0
    }
}

// Common path constants
const PATH_CFG: PathSegments = PathSegments::new(&["cfg"]);
const PATH_TEST: PathSegments = PathSegments::new(&["test"]);
const PATH_ANY: PathSegments = PathSegments::new(&["any"]);
const PATH_ALL: PathSegments = PathSegments::new(&["all"]);
const PATH_NOT: PathSegments = PathSegments::new(&["not"]);
const PATH_CFG_ATTR: PathSegments = PathSegments::new(&["cfg_attr"]);
const PATH_ALLOW: PathSegments = PathSegments::new(&["allow"]);
const PATH_DOCTEST: PathSegments = PathSegments::new(&["doctest"]);
const PATH_UNIX: PathSegments = PathSegments::new(&["unix"]);
const PATH_DEAD_CODE: PathSegments = PathSegments::new(&["dead_code"]);

fn path_from_segments(segments: PathSegments) -> Path {
    let path_segments = segments
        .as_slice()
        .iter()
        .map(|segment| PathSegment::from_ident(Ident::from_str(segment)))
        .collect::<Vec<_>>()
        .into();

    Path {
        span: DUMMY_SP,
        segments: path_segments,
        tokens: None,
    }
}

fn hir_attribute_from_segments(segments: PathSegments) -> hir::Attribute {
    let path_segments = segments
        .as_slice()
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
        style: rustc_ast::AttrStyle::Outer,
        span: DUMMY_SP,
    };

    hir::Attribute::Unparsed(Box::new(attr_item))
}

/// Verify that `convert_attribute` preserves path segments for attributes.
#[rstest]
#[case::multi_segment(PathSegments::new(&["tokio", "test"]))]
#[case::single_segment(PathSegments::new(&["rstest"]))]
fn convert_attribute_preserves_path_segments(#[case] segments: PathSegments) {
    create_default_session_globals_then(|| {
        assert_converts_path(segments);
    });
}

fn meta_word(segments: PathSegments) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::Word,
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

fn meta_list(segments: PathSegments, children: Vec<MetaItemInner>) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::List(children.into()),
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

fn meta_inner(meta: MetaItem) -> MetaItemInner {
    MetaItemInner::MetaItem(meta)
}

// ---------------------------------------------------------------------------
// cfg pattern helpers
// ---------------------------------------------------------------------------

/// Builds `cfg(any(...))`.
fn cfg_any(items: Vec<MetaItem>) -> MetaItem {
    meta_list(
        PATH_CFG,
        vec![meta_inner(meta_list(
            PATH_ANY,
            items.into_iter().map(meta_inner).collect(),
        ))],
    )
}

/// Builds `cfg(all(...))`.
fn cfg_all(items: Vec<MetaItem>) -> MetaItem {
    meta_list(
        PATH_CFG,
        vec![meta_inner(meta_list(
            PATH_ALL,
            items.into_iter().map(meta_inner).collect(),
        ))],
    )
}

/// Builds `cfg(not(...))`.
fn cfg_not(item: MetaItem) -> MetaItem {
    meta_list(
        PATH_CFG,
        vec![meta_inner(meta_list(PATH_NOT, vec![meta_inner(item)]))],
    )
}

/// Builds `cfg_attr(condition, attribute)`.
fn cfg_attr(condition: MetaItem, attribute: MetaItem) -> MetaItem {
    meta_list(
        PATH_CFG_ATTR,
        vec![meta_inner(condition), meta_inner(attribute)],
    )
}

/// Builds simple `cfg(path)` style attributes.
fn cfg_simple(segments: PathSegments) -> MetaItem {
    meta_list(PATH_CFG, vec![meta_inner(meta_word(segments))])
}

// ---------------------------------------------------------------------------
// MetaItem builders for parameterised tests
// ---------------------------------------------------------------------------

/// Builds `cfg(any(test, doctest))`.
fn build_cfg_any_test_doctest() -> MetaItem {
    cfg_any(vec![meta_word(PATH_TEST), meta_word(PATH_DOCTEST)])
}

/// Builds `cfg(all(test, unix))`.
fn build_cfg_all_test_unix() -> MetaItem {
    cfg_all(vec![meta_word(PATH_TEST), meta_word(PATH_UNIX)])
}

/// Builds `cfg(not(test))`.
fn build_cfg_not_test() -> MetaItem {
    cfg_not(meta_word(PATH_TEST))
}

/// Builds `cfg_attr(test, cfg(test))`.
fn build_cfg_attr_test_cfg_test() -> MetaItem {
    cfg_attr(meta_word(PATH_TEST), cfg_simple(PATH_TEST))
}

/// Builds `cfg_attr(test, allow(dead_code))`.
fn build_cfg_attr_test_allow() -> MetaItem {
    cfg_attr(
        meta_word(PATH_TEST),
        meta_list(PATH_ALLOW, vec![meta_inner(meta_word(PATH_DEAD_CODE))]),
    )
}

/// Helper function to test `meta_contains_test_cfg` behaviour.
fn assert_meta_test_cfg(meta: MetaItem, expected: bool) {
    assert_eq!(meta_contains_test_cfg(&meta), expected);
}

/// Asserts that `convert_attribute` preserves path segments for the given
/// attribute path. Must be called within `create_default_session_globals_then`.
fn assert_converts_path(segments: PathSegments) {
    let hir_attr = hir_attribute_from_segments(segments);
    let attribute = convert_attribute(&hir_attr);

    assert_eq!(attribute.kind(), AttributeKind::Outer);
    let converted_segments = attribute
        .path()
        .segments()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(converted_segments.as_slice(), segments.as_slice());
}

/// Verify `cfg(any(test, doctest))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_any_test_doctest() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_any_test_doctest(), true);
    });
}

/// Verify `cfg(all(test, unix))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_all_test_unix() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_all_test_unix(), true);
    });
}

/// Verify `cfg(not(test))` is NOT detected as a test context (negated).
#[test]
fn meta_contains_test_cfg_not_test() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_not_test(), false);
    });
}

/// Verify `cfg_attr(test, cfg(test))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_attr_test_cfg_test() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_attr_test_cfg_test(), true);
    });
}

/// Verify `cfg_attr(test, allow(dead_code))` is NOT detected as a test context.
#[test]
fn meta_contains_test_cfg_attr_test_allow() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_attr_test_allow(), false);
    });
}

/// Verify that `convert_attribute` handles parsed attributes without panicking.
///
/// Parsed attributes (e.g., `#[must_use]`) are pre-processed by rustc and don't
/// have an accessible path. Calling `path()` on them would panic.
#[test]
fn convert_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    let attribute = convert_attribute(&parsed_attr);

    // Should return a placeholder "parsed" path instead of panicking.
    assert_eq!(attribute.path().segments(), &["parsed".to_string()]);
    assert_eq!(attribute.kind(), AttributeKind::Outer);
}

/// Verify that `is_cfg_test_attribute` handles parsed attributes without panicking.
#[test]
fn is_cfg_test_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    // Should return false (not a cfg(test) attribute) without panicking.
    assert!(!is_cfg_test_attribute(&parsed_attr));
}
