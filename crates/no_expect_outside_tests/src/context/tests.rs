//! Unit tests for context conversion and attribute detection.
//!
//! Verifies HIR attribute conversion to `common::Attribute` and `cfg(test)`
//! detection for both parsed and unparsed attribute variants.

use super::{convert_attribute, is_cfg_test_attribute, meta_contains_test_cfg};
use common::AttributeKind;
use rustc_ast::ast::{MetaItem, MetaItemInner, MetaItemKind, Path, PathSegment, Safety};
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_span::symbol::Ident;
use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};

fn path_from_segments(segments: &[&str]) -> Path {
    let path_segments = segments
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
        style: rustc_ast::AttrStyle::Outer,
        span: DUMMY_SP,
    };

    hir::Attribute::Unparsed(Box::new(attr_item))
}

/// Verify that `convert_attribute` preserves path segments for multi-segment
/// attributes like `#[tokio::test]`.
#[test]
fn convert_attribute_preserves_multi_segment_path() {
    create_default_session_globals_then(|| {
        assert_converts_path(&["tokio", "test"]);
    });
}

/// Verify that `convert_attribute` preserves path segments for single-segment
/// attributes like `#[rstest]`.
#[test]
fn convert_attribute_preserves_single_segment_path() {
    create_default_session_globals_then(|| {
        assert_converts_path(&["rstest"]);
    });
}

fn meta_word(segments: &[&str]) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::Word,
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

fn meta_list(segments: &[&str], children: Vec<MetaItemInner>) -> MetaItem {
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

/// Asserts that `convert_attribute` preserves path segments for the given
/// attribute path. Must be called within `create_default_session_globals_then`.
fn assert_converts_path(segments: &[&str]) {
    let hir_attr = hir_attribute_from_segments(segments);
    let attribute = convert_attribute(&hir_attr);

    assert_eq!(attribute.kind(), AttributeKind::Outer);
    let converted_segments = attribute
        .path()
        .segments()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(converted_segments.as_slice(), segments);
}

/// Asserts that `meta_contains_test_cfg` returns the expected result for the
/// given `MetaItem`. Must be called within `create_default_session_globals_then`.
fn assert_meta_test_cfg(meta: MetaItem, expected: bool) {
    assert_eq!(meta_contains_test_cfg(&meta), expected);
}

/// Verify `cfg(any(test, doctest))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_any_test_doctest() {
    create_default_session_globals_then(|| {
        let meta = meta_list(
            &["cfg"],
            vec![meta_inner(meta_list(
                &["any"],
                vec![
                    meta_inner(meta_word(&["test"])),
                    meta_inner(meta_word(&["doctest"])),
                ],
            ))],
        );
        assert_meta_test_cfg(meta, true);
    });
}

/// Verify `cfg(all(test, unix))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_all_test_unix() {
    create_default_session_globals_then(|| {
        let meta = meta_list(
            &["cfg"],
            vec![meta_inner(meta_list(
                &["all"],
                vec![
                    meta_inner(meta_word(&["test"])),
                    meta_inner(meta_word(&["unix"])),
                ],
            ))],
        );
        assert_meta_test_cfg(meta, true);
    });
}

/// Verify `cfg(not(test))` is NOT detected as a test context (negated).
#[test]
fn meta_contains_test_cfg_not_test() {
    create_default_session_globals_then(|| {
        let meta = meta_list(
            &["cfg"],
            vec![meta_inner(meta_list(
                &["not"],
                vec![meta_inner(meta_word(&["test"]))],
            ))],
        );
        assert_meta_test_cfg(meta, false);
    });
}

/// Verify `cfg_attr(test, cfg(test))` is detected as a test context.
#[test]
fn meta_contains_test_cfg_attr_test_cfg_test() {
    create_default_session_globals_then(|| {
        let meta = meta_list(
            &["cfg_attr"],
            vec![
                meta_inner(meta_word(&["test"])),
                meta_inner(meta_list(&["cfg"], vec![meta_inner(meta_word(&["test"]))])),
            ],
        );
        assert_meta_test_cfg(meta, true);
    });
}

/// Verify `cfg_attr(test, allow(dead_code))` is NOT detected as a test context.
#[test]
fn meta_contains_test_cfg_attr_test_allow() {
    create_default_session_globals_then(|| {
        let meta = meta_list(
            &["cfg_attr"],
            vec![
                meta_inner(meta_word(&["test"])),
                meta_inner(meta_list(
                    &["allow"],
                    vec![meta_inner(meta_word(&["dead_code"]))],
                )),
            ],
        );
        assert_meta_test_cfg(meta, false);
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
