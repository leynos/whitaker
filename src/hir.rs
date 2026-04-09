//! Helpers for working with HIR constructs shared across Whitaker lints.

use std::collections::HashSet;

use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};
use whitaker_common::{Attribute, AttributeKind, AttributePath};

/// Returns the body span for an inline or file-backed module.
///
/// The helper mirrors the idiom used by multiple lints: prefer the inner
/// module span when present, otherwise fall back to the definition span, and
/// finally the item span. Callers may further adjust the returned span (for
/// example, shrink it to the opening brace) depending on their diagnostic
/// needs.
#[must_use]
pub fn module_body_span<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx hir::Item<'tcx>,
    module: &hir::Mod<'tcx>,
) -> Span {
    let inner = module.spans.inner_span;
    if !inner.is_dummy() {
        return inner;
    }

    let def_span = cx.tcx.def_span(item.owner_id.to_def_id());
    if !def_span.is_dummy() {
        return def_span;
    }

    item.span
}

/// Produces the span covering the module header (`mod foo {`).
#[must_use]
pub fn module_header_span(item_span: Span, ident_span: Span) -> Span {
    item_span.with_hi(ident_span.hi())
}

/// Returns whether any HIR attribute resolves to a recognized test marker.
#[must_use]
pub fn has_test_like_hir_attributes(
    attrs: &[hir::Attribute],
    additional: &[AttributePath],
) -> bool {
    attrs
        .iter()
        .filter_map(attribute_from_hir)
        .any(|attribute| attribute.is_test_like_with(additional))
}

fn attribute_from_hir(attr: &hir::Attribute) -> Option<Attribute> {
    Some(Attribute::new(attribute_path(attr)?, attribute_kind(attr)))
}

fn attribute_path(attr: &hir::Attribute) -> Option<AttributePath> {
    let hir::Attribute::Unparsed(_) = attr else {
        return None;
    };

    let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
    let first = names.next()?;
    Some(AttributePath::new(std::iter::once(first).chain(names)))
}

fn attribute_kind(attr: &hir::Attribute) -> AttributeKind {
    match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    }
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Unparsed(item) => item.style,
        hir::Attribute::Parsed(HirAttributeKind::DocComment { style, .. }) => *style,
        hir::Attribute::Parsed(_) => AttrStyle::Outer,
    }
}

/// Collects all functions that the `rustc --test` harness identifies as tests.
///
/// The test harness synthesises a sibling `const` descriptor with the same name
/// and source span as each test function. This function scans for those
/// descriptors to recover test-function identity after the harness has consumed
/// the original `#[test]` attributes.
///
/// Items are grouped by their parent module so that descriptor matching only
/// considers true siblings, not unrelated items from other modules.
#[must_use]
pub fn collect_harness_test_functions(cx: &LateContext<'_>) -> HashSet<hir::HirId> {
    let root_items: Vec<_> = cx
        .tcx
        .hir_crate_items(())
        .free_items()
        .map(|id| cx.tcx.hir_item(id))
        .filter(|item| is_crate_root_item(cx, item))
        .collect();
    let mut marked = HashSet::new();
    collect_in_item_group(cx, &root_items, &mut marked);
    marked
}

/// Returns `true` when the item's immediate parent is the crate root rather
/// than a nested module.
fn is_crate_root_item(cx: &LateContext<'_>, item: &hir::Item<'_>) -> bool {
    cx.tcx
        .hir_parent_iter(item.hir_id())
        .next()
        .is_some_and(|(_, node)| matches!(node, hir::Node::Crate(_)))
}

fn collect_in_item_group<'tcx>(
    cx: &LateContext<'tcx>,
    items: &[&'tcx hir::Item<'tcx>],
    marked: &mut HashSet<hir::HirId>,
) {
    for item in items
        .iter()
        .copied()
        .filter(|item| matches!(item.kind, hir::ItemKind::Fn { .. }))
    {
        let Some(ident) = item.kind.ident() else {
            continue;
        };

        if items.iter().any(|sibling| {
            is_harness_test_descriptor(item.hir_id(), ident.name, item.span, sibling)
        }) {
            marked.insert(item.hir_id());
        }
    }

    recurse_into_submodules(cx, items, marked);
}

fn recurse_into_submodules<'tcx>(
    cx: &LateContext<'tcx>,
    items: &[&'tcx hir::Item<'tcx>],
    marked: &mut HashSet<hir::HirId>,
) {
    for item in items {
        let hir::ItemKind::Mod(_, module) = item.kind else {
            continue;
        };
        let module_items: Vec<_> = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .collect();
        collect_in_item_group(cx, &module_items, marked);
    }
}

/// The `--test` harness synthesises a `const` with the same name and source
/// range as the test function.
fn is_harness_test_descriptor(
    fn_hir_id: hir::HirId,
    fn_name: Symbol,
    fn_span: Span,
    sibling: &hir::Item<'_>,
) -> bool {
    sibling.hir_id() != fn_hir_id
        && matches!(sibling.kind, hir::ItemKind::Const(..))
        && sibling
            .kind
            .ident()
            .is_some_and(|ident| ident.name == fn_name && sibling.span.source_equal(fn_span))
}
