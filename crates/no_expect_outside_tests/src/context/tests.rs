//! Unit tests for context conversion and attribute detection.
//!
//! Verifies HIR attribute conversion to `whitaker_common::Attribute` and `cfg(test)`
//! detection for both parsed and unparsed attribute variants.

#[cfg(feature = "dylint-driver")]
use super::{convert_attribute, has_test_ancestry, is_cfg_test_attribute, meta_contains_test_cfg};
#[cfg(feature = "dylint-driver")]
use rstest::rstest;
#[cfg(feature = "dylint-driver")]
use rustc_ast::ast::{DelimArgs, MetaItem, MetaItemInner, MetaItemKind, Path, PathSegment, Safety};
#[cfg(feature = "dylint-driver")]
use rustc_ast::token::{Delimiter, IdentIsRaw, TokenKind};
#[cfg(feature = "dylint-driver")]
use rustc_ast::tokenstream::{DelimSpan, TokenStream, TokenTree};
#[cfg(feature = "dylint-driver")]
use rustc_hir as hir;
#[cfg(feature = "dylint-driver")]
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
#[cfg(feature = "dylint-driver")]
use rustc_span::symbol::Ident;
#[cfg(feature = "dylint-driver")]
use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};
#[cfg(feature = "dylint-driver")]
use whitaker_common::{AttributeKind, AttributePath, PARSED_ATTRIBUTE_PLACEHOLDER};

/// Type-safe wrapper for AST path segments.
#[cfg(feature = "dylint-driver")]
#[derive(Debug, Clone, Copy)]
struct PathSegments(&'static [&'static str]);

#[cfg(feature = "dylint-driver")]
impl PathSegments {
    const fn new(segments: &'static [&'static str]) -> Self {
        Self(segments)
    }
}

#[cfg(feature = "dylint-driver")]
impl AsRef<[&'static str]> for PathSegments {
    fn as_ref(&self) -> &[&'static str] {
        self.0
    }
}

#[cfg(feature = "dylint-driver")]
#[derive(Clone, Copy, Debug)]
enum AttributeFixture {
    None,
    Allow,
    CfgTest,
    BuiltInTest,
    CustomTest,
    CfgAndBuiltInTest,
}

#[cfg(feature = "dylint-driver")]
#[derive(Clone, Copy, Debug)]
struct HasTestAncestryCase {
    has_test_context_ancestry: bool,
    attr_fixture: AttributeFixture,
    is_function_item: bool,
    include_custom_attribute: bool,
    expected: bool,
}

// Common path constants
#[cfg(feature = "dylint-driver")]
const PATH_CFG: PathSegments = PathSegments::new(&["cfg"]);
#[cfg(feature = "dylint-driver")]
const PATH_TEST: PathSegments = PathSegments::new(&["test"]);
#[cfg(feature = "dylint-driver")]
const PATH_ANY: PathSegments = PathSegments::new(&["any"]);
#[cfg(feature = "dylint-driver")]
const PATH_ALL: PathSegments = PathSegments::new(&["all"]);
#[cfg(feature = "dylint-driver")]
const PATH_NOT: PathSegments = PathSegments::new(&["not"]);
#[cfg(feature = "dylint-driver")]
const PATH_CFG_ATTR: PathSegments = PathSegments::new(&["cfg_attr"]);
#[cfg(feature = "dylint-driver")]
const PATH_ALLOW: PathSegments = PathSegments::new(&["allow"]);
#[cfg(feature = "dylint-driver")]
const PATH_DOCTEST: PathSegments = PathSegments::new(&["doctest"]);
#[cfg(feature = "dylint-driver")]
const PATH_UNIX: PathSegments = PathSegments::new(&["unix"]);
#[cfg(feature = "dylint-driver")]
const PATH_DEAD_CODE: PathSegments = PathSegments::new(&["dead_code"]);

#[cfg(feature = "dylint-driver")]
fn path_from_segments(segments: PathSegments) -> Path {
    let path_segments = segments
        .as_ref()
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

#[cfg(feature = "dylint-driver")]
fn hir_attribute_from_segments(segments: PathSegments) -> hir::Attribute {
    let path_segments = segments
        .as_ref()
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

#[cfg(feature = "dylint-driver")]
fn hir_cfg_test_attribute() -> hir::Attribute {
    let attr_item = hir::AttrItem {
        path: hir::AttrPath {
            segments: vec![Ident::from_str("cfg")].into_boxed_slice(),
            span: DUMMY_SP,
        },
        args: hir::AttrArgs::Delimited(DelimArgs {
            dspan: DelimSpan::from_single(DUMMY_SP),
            delim: Delimiter::Parenthesis,
            tokens: TokenStream::new(vec![TokenTree::token_alone(
                TokenKind::Ident(rustc_span::sym::test, IdentIsRaw::No),
                DUMMY_SP,
            )]),
        }),
        id: hir::HashIgnoredAttrId {
            attr_id: AttrId::from_u32(1),
        },
        style: rustc_ast::AttrStyle::Outer,
        span: DUMMY_SP,
    };

    hir::Attribute::Unparsed(Box::new(attr_item))
}

/// Verify that `convert_attribute` preserves path segments for attributes.
#[cfg(feature = "dylint-driver")]
#[rstest]
#[case::multi_segment(PathSegments::new(&["tokio", "test"]))]
#[case::single_segment(PathSegments::new(&["rstest"]))]
fn convert_attribute_preserves_path_segments(#[case] segments: PathSegments) {
    create_default_session_globals_then(|| {
        assert_converts_path(segments);
    });
}

#[cfg(feature = "dylint-driver")]
fn meta_word(segments: PathSegments) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::Word,
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

#[cfg(feature = "dylint-driver")]
fn meta_list(segments: PathSegments, children: Vec<MetaItemInner>) -> MetaItem {
    MetaItem {
        path: path_from_segments(segments),
        kind: MetaItemKind::List(children.into()),
        span: DUMMY_SP,
        unsafety: Safety::Default,
    }
}

#[cfg(feature = "dylint-driver")]
fn meta_inner(meta: MetaItem) -> MetaItemInner {
    MetaItemInner::MetaItem(meta)
}

// ---------------------------------------------------------------------------
// cfg pattern helpers
// ---------------------------------------------------------------------------

/// Builds `cfg(any(...))`.
#[cfg(feature = "dylint-driver")]
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
#[cfg(feature = "dylint-driver")]
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
#[cfg(feature = "dylint-driver")]
fn cfg_not(item: MetaItem) -> MetaItem {
    meta_list(
        PATH_CFG,
        vec![meta_inner(meta_list(PATH_NOT, vec![meta_inner(item)]))],
    )
}

/// Builds `cfg_attr(condition, attribute)`.
#[cfg(feature = "dylint-driver")]
fn cfg_attr(condition: MetaItem, attribute: MetaItem) -> MetaItem {
    meta_list(
        PATH_CFG_ATTR,
        vec![meta_inner(condition), meta_inner(attribute)],
    )
}

/// Builds simple `cfg(path)` style attributes.
#[cfg(feature = "dylint-driver")]
fn cfg_simple(segments: PathSegments) -> MetaItem {
    meta_list(PATH_CFG, vec![meta_inner(meta_word(segments))])
}

// ---------------------------------------------------------------------------
// MetaItem builders for parameterised tests
// ---------------------------------------------------------------------------

/// Builds `cfg(any(test, doctest))`.
#[cfg(feature = "dylint-driver")]
fn build_cfg_any_test_doctest() -> MetaItem {
    cfg_any(vec![meta_word(PATH_TEST), meta_word(PATH_DOCTEST)])
}

/// Builds `cfg(all(test, unix))`.
#[cfg(feature = "dylint-driver")]
fn build_cfg_all_test_unix() -> MetaItem {
    cfg_all(vec![meta_word(PATH_TEST), meta_word(PATH_UNIX)])
}

/// Builds `cfg(not(test))`.
#[cfg(feature = "dylint-driver")]
fn build_cfg_not_test() -> MetaItem {
    cfg_not(meta_word(PATH_TEST))
}

/// Builds `cfg_attr(test, cfg(test))`.
#[cfg(feature = "dylint-driver")]
fn build_cfg_attr_test_cfg_test() -> MetaItem {
    cfg_attr(meta_word(PATH_TEST), cfg_simple(PATH_TEST))
}

/// Builds `cfg_attr(test, allow(dead_code))`.
#[cfg(feature = "dylint-driver")]
fn build_cfg_attr_test_allow() -> MetaItem {
    cfg_attr(
        meta_word(PATH_TEST),
        meta_list(PATH_ALLOW, vec![meta_inner(meta_word(PATH_DEAD_CODE))]),
    )
}

/// Helper function to test `meta_contains_test_cfg` behaviour.
/// Must be called within `create_default_session_globals_then`.
#[cfg(feature = "dylint-driver")]
fn assert_meta_test_cfg(meta: MetaItem, expected: bool) {
    assert_eq!(meta_contains_test_cfg(&meta), expected);
}

/// Asserts that `convert_attribute` preserves path segments for the given
/// attribute path. Must be called within `create_default_session_globals_then`.
#[cfg(feature = "dylint-driver")]
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
    assert_eq!(converted_segments.as_slice(), segments.as_ref());
}

#[cfg(feature = "dylint-driver")]
fn build_test_attrs(fixture: AttributeFixture) -> Vec<hir::Attribute> {
    match fixture {
        AttributeFixture::None => Vec::new(),
        AttributeFixture::Allow => {
            vec![hir_attribute_from_segments(PathSegments::new(&["allow"]))]
        }
        AttributeFixture::CfgTest => vec![hir_cfg_test_attribute()],
        AttributeFixture::BuiltInTest => vec![hir_attribute_from_segments(PATH_TEST)],
        AttributeFixture::CustomTest => vec![hir_attribute_from_segments(PathSegments::new(&[
            "my_framework",
            "test",
        ]))],
        AttributeFixture::CfgAndBuiltInTest => {
            vec![
                hir_cfg_test_attribute(),
                hir_attribute_from_segments(PATH_TEST),
            ]
        }
    }
}

#[cfg(feature = "dylint-driver")]
fn build_additional_test_attributes(include_custom: bool) -> Vec<AttributePath> {
    if include_custom {
        vec![AttributePath::from("my_framework::test")]
    } else {
        Vec::new()
    }
}

/// Verify `has_test_ancestry` for propagation, `cfg(test)`, and function-item
/// marker detection.
#[cfg(feature = "dylint-driver")]
#[rstest]
#[case::prior_detection_carries_forward(HasTestAncestryCase {
    has_test_context_ancestry: true,
    attr_fixture: AttributeFixture::None,
    is_function_item: false,
    include_custom_attribute: false,
    expected: true,
})]
#[case::cfg_test_attribute_detected(HasTestAncestryCase {
    has_test_context_ancestry: false,
    attr_fixture: AttributeFixture::CfgTest,
    is_function_item: false,
    include_custom_attribute: false,
    expected: true,
})]
#[case::built_in_test_attribute_on_function_item(HasTestAncestryCase {
    has_test_context_ancestry: false,
    attr_fixture: AttributeFixture::BuiltInTest,
    is_function_item: true,
    include_custom_attribute: false,
    expected: true,
})]
#[case::configured_test_attribute_on_function_item(HasTestAncestryCase {
    has_test_context_ancestry: false,
    attr_fixture: AttributeFixture::CustomTest,
    is_function_item: true,
    include_custom_attribute: true,
    expected: true,
})]
#[case::all_detection_paths_together(HasTestAncestryCase {
    has_test_context_ancestry: true,
    attr_fixture: AttributeFixture::CfgAndBuiltInTest,
    is_function_item: true,
    include_custom_attribute: true,
    expected: true,
})]
#[case::negative_case(HasTestAncestryCase {
    has_test_context_ancestry: false,
    attr_fixture: AttributeFixture::Allow,
    is_function_item: false,
    include_custom_attribute: false,
    expected: false,
})]
fn has_test_ancestry_detects_test_context(#[case] case: HasTestAncestryCase) {
    create_default_session_globals_then(|| {
        let attrs = build_test_attrs(case.attr_fixture);
        let additional_test_attributes =
            build_additional_test_attributes(case.include_custom_attribute);

        assert_eq!(
            has_test_ancestry(
                case.has_test_context_ancestry,
                &attrs,
                case.is_function_item,
                &additional_test_attributes,
            ),
            case.expected,
        );
    });
}

/// Verify `cfg(any(test, doctest))` is detected as a test context.
#[cfg(feature = "dylint-driver")]
#[test]
fn meta_contains_test_cfg_any_test_doctest() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_any_test_doctest(), true);
    });
}

/// Verify `cfg(all(test, unix))` is detected as a test context.
#[cfg(feature = "dylint-driver")]
#[test]
fn meta_contains_test_cfg_all_test_unix() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_all_test_unix(), true);
    });
}

/// Verify `cfg(not(test))` is NOT detected as a test context (negated).
#[cfg(feature = "dylint-driver")]
#[test]
fn meta_contains_test_cfg_not_test() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_not_test(), false);
    });
}

/// Verify `cfg_attr(test, cfg(test))` is detected as a test context.
#[cfg(feature = "dylint-driver")]
#[test]
fn meta_contains_test_cfg_attr_test_cfg_test() {
    create_default_session_globals_then(|| {
        assert_meta_test_cfg(build_cfg_attr_test_cfg_test(), true);
    });
}

/// Verify `cfg_attr(test, allow(dead_code))` is NOT detected as a test context.
#[cfg(feature = "dylint-driver")]
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
#[cfg(feature = "dylint-driver")]
#[test]
fn convert_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    let attribute = convert_attribute(&parsed_attr);

    // Should return a placeholder "parsed" path instead of panicking.
    assert_eq!(
        attribute.path().segments(),
        &[PARSED_ATTRIBUTE_PLACEHOLDER.to_string()]
    );
    assert_eq!(attribute.kind(), AttributeKind::Outer);
}

/// Verify that `is_cfg_test_attribute` handles parsed attributes without panicking.
#[cfg(feature = "dylint-driver")]
#[test]
fn is_cfg_test_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    // Should return false (not a cfg(test) attribute) without panicking.
    assert!(!is_cfg_test_attribute(&parsed_attr));
}
