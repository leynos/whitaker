//! `MetaItem` builders for the `cfg(test)` detection tests.

use rustc_ast::ast::{MetaItem, MetaItemInner, MetaItemKind, Safety};
use rustc_span::DUMMY_SP;

use super::{
    PATH_ALL,
    PATH_ALLOW,
    PATH_ANY,
    PATH_CFG,
    PATH_CFG_ATTR,
    PATH_DEAD_CODE,
    PATH_DOCTEST,
    PATH_NOT,
    PATH_TEST,
    PATH_UNIX,
    PathSegments,
    path_from_segments,
};
use crate::context::meta_contains_test_cfg;

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

fn meta_inner(meta: MetaItem) -> MetaItemInner { MetaItemInner::MetaItem(meta) }

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
// MetaItem builders for parameterized tests
// ---------------------------------------------------------------------------

/// Builds `cfg(any(test, doctest))`.
pub(super) fn build_cfg_any_test_doctest() -> MetaItem {
    cfg_any(vec![meta_word(PATH_TEST), meta_word(PATH_DOCTEST)])
}

/// Builds `cfg(all(test, unix))`.
pub(super) fn build_cfg_all_test_unix() -> MetaItem {
    cfg_all(vec![meta_word(PATH_TEST), meta_word(PATH_UNIX)])
}

/// Builds `cfg(not(test))`.
pub(super) fn build_cfg_not_test() -> MetaItem { cfg_not(meta_word(PATH_TEST)) }

/// Builds `cfg_attr(test, cfg(test))`.
pub(super) fn build_cfg_attr_test_cfg_test() -> MetaItem {
    cfg_attr(meta_word(PATH_TEST), cfg_simple(PATH_TEST))
}

/// Builds `cfg_attr(test, allow(dead_code))`.
pub(super) fn build_cfg_attr_test_allow() -> MetaItem {
    cfg_attr(
        meta_word(PATH_TEST),
        meta_list(PATH_ALLOW, vec![meta_inner(meta_word(PATH_DEAD_CODE))]),
    )
}

/// Helper function to test `meta_contains_test_cfg` behaviour.
/// Must be called within `create_default_session_globals_then`.
pub(super) fn assert_meta_test_cfg(meta: &MetaItem, expected: bool) {
    assert_eq!(
        meta_contains_test_cfg(meta),
        expected,
        "meta item should report test-cfg detection as {expected}"
    );
}
