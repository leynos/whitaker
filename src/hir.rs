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
/// The test harness synthesizes a sibling `const` descriptor with the same name,
/// source span, and parent module as each test function. This function performs
/// a single flat scan over all crate items to match descriptors with functions,
/// checking parent equality to ensure true siblings.
#[must_use]
pub fn collect_harness_test_functions(cx: &LateContext<'_>) -> HashSet<hir::HirId> {
    let all_items = cx.tcx.hir_crate_items(());

    // First pass: collect all const descriptors with their parent module.
    // Key by (name, parent_module) to group descriptors by their declaring module.
    let mut descriptors: HashMap<(rustc_span::Symbol, hir::HirId), Vec<(hir::HirId, Span)>> =
        HashMap::new();

    for item_id in all_items.free_items() {
        let item = cx.tcx.hir_item(item_id);
        if !matches!(item.kind, hir::ItemKind::Const(..)) {
            continue;
        }
        let Some(ident) = item.kind.ident() else {
            continue;
        };
        let parent = cx.tcx.hir_get_parent_item(item.hir_id()).into();
        descriptors
            .entry((ident.name, parent))
            .or_default()
            .push((item.hir_id(), item.span));
    }

    // Second pass: find functions with matching descriptors in the same module.
    let mut marked = HashSet::new();
    for item_id in all_items.free_items() {
        let item = cx.tcx.hir_item(item_id);
        if !matches!(item.kind, hir::ItemKind::Fn { .. }) {
            continue;
        }
        let Some(ident) = item.kind.ident() else {
            continue;
        };

        let parent = cx.tcx.hir_get_parent_item(item.hir_id()).into();
        let Some(candidates) = descriptors.get(&(ident.name, parent)) else {
            continue;
        };

        for &(desc_id, desc_span) in candidates {
            if desc_id != item.hir_id() && desc_span.source_equal(item.span) {
                marked.insert(item.hir_id());
                break;
            }
        }
    }

    marked
}
