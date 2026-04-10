//! Helpers for working with HIR constructs shared across Whitaker lints.

use std::collections::{HashMap, HashSet};

use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::LateContext;
use rustc_span::Span;
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
/// The test harness synthesizes a sibling `const` descriptor with the same name
/// and source span as each test function. This function scans for those
/// descriptors to recover test-function identity after the harness has consumed
/// the original `#[test]` attributes.
///
/// Items are grouped by their parent module so that descriptor matching only
/// considers true siblings, not unrelated items from other modules.
#[must_use]
pub fn collect_harness_test_functions(cx: &LateContext<'_>) -> HashSet<hir::HirId> {
    // Start traversal from the crate root module rather than scanning all items.
    // The crate owner nodes contain the root module, whose item_ids give us
    // direct children without needing to filter free_items().
    let root_module = cx.tcx.hir_owner_nodes(hir::CRATE_OWNER_ID).node();
    let hir::OwnerNode::Crate(crate_mod) = root_module else {
        return HashSet::new();
    };

    let root_items: Vec<_> = crate_mod
        .item_ids
        .iter()
        .map(|id| cx.tcx.hir_item(*id))
        .collect();

    let mut marked = HashSet::new();
    collect_in_item_group(cx, &root_items, &mut marked);
    marked
}

fn collect_in_item_group<'tcx>(
    cx: &LateContext<'tcx>,
    items: &[&'tcx hir::Item<'tcx>],
    marked: &mut HashSet<hir::HirId>,
) {
    // First pass: build a lookup of harness test descriptors by name.
    // Descriptors are const items synthesized by the --test harness with the
    // same name and source-equal span as their corresponding test functions.
    let mut descriptors_by_name: HashMap<rustc_span::Symbol, Vec<(hir::HirId, Span)>> =
        HashMap::new();
    for item in items.iter().copied() {
        if !matches!(item.kind, hir::ItemKind::Const(..)) {
            continue;
        }
        let Some(ident) = item.kind.ident() else {
            continue;
        };
        descriptors_by_name
            .entry(ident.name)
            .or_default()
            .push((item.hir_id(), item.span));
    }

    // Second pass: check each function against the descriptor lookup.
    // This is O(functions + descriptors × name_collisions) which in practice
    // is much faster than O(functions × all_descriptors) since most names are
    // unique.
    for item in items
        .iter()
        .copied()
        .filter(|item| matches!(item.kind, hir::ItemKind::Fn { .. }))
    {
        let Some(ident) = item.kind.ident() else {
            continue;
        };

        let Some(candidates) = descriptors_by_name.get(&ident.name) else {
            continue;
        };

        for &(desc_id, desc_span) in candidates {
            if desc_id != item.hir_id() && desc_span.source_equal(item.span) {
                marked.insert(item.hir_id());
                break;
            }
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
