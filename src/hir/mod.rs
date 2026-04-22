//! Helpers for working with HIR constructs shared across Whitaker lints.

use std::collections::HashSet;

use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::LateContext;
use rustc_span::Span;
use whitaker_common::{Attribute, AttributeKind, AttributePath, SpanRecoveryFrame};

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

/// Internal iterator over a `Span` callsite chain.
///
/// The walk yields the original span first, then follows `source_callsite()`
/// while the current span still comes from expansion. It stops when it reaches
/// a dummy span, a non-expansion span, or a callsite that is dummy or makes no
/// progress.
fn walk_span_chain(start: Span) -> impl Iterator<Item = Span> {
    let mut current = Some(start);

    std::iter::from_fn(move || {
        let span = current?;
        if span.is_dummy() {
            current = None;
            return None;
        }

        current = if span.from_expansion() {
            let next = span.source_callsite();
            if next.is_dummy() || next == span {
                None
            } else {
                Some(next)
            }
        } else {
            None
        };

        Some(span)
    })
}

/// Collects ordered [`SpanRecoveryFrame`] values for a `rustc_span::Span`.
///
/// The first frame is always the original span when it is not dummy. Later
/// frames follow the `source_callsite()` chain produced by
/// [`walk_span_chain`], preserving each yielded span together with its
/// `from_expansion()` state in a [`SpanRecoveryFrame`].
///
/// The walk stops when it reaches a dummy span, a user-editable span, or a
/// `source_callsite()` value that is dummy or makes no further progress.
#[must_use]
pub fn span_recovery_frames(span: Span) -> Vec<SpanRecoveryFrame<Span>> {
    walk_span_chain(span)
        .map(|current| SpanRecoveryFrame::new(current, current.from_expansion()))
        .collect()
}

/// Recovers the first user-editable HIR span from a macro expansion chain.
#[must_use]
pub fn recover_user_editable_hir_span(span: Span) -> Option<Span> {
    walk_span_chain(span).find(|current| !current.from_expansion())
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

/// Extracts `(hir_id, name, parent, span)` from `item` when `kind_matches` returns `true`.
fn item_components<'tcx>(
    cx: &LateContext<'tcx>,
    item: &hir::Item<'tcx>,
    kind_matches: impl Fn(&hir::ItemKind<'tcx>) -> bool,
) -> Option<(hir::HirId, rustc_span::Symbol, hir::HirId, Span)> {
    if !kind_matches(&item.kind) {
        return None;
    }
    let ident = item.kind.ident()?;
    let parent: hir::HirId = cx.tcx.hir_get_parent_item(item.hir_id()).into();
    Some((item.hir_id(), ident.name, parent, item.span))
}

/// Collects all functions that the `rustc --test` harness identifies as tests.
///
/// The test harness synthesizes a sibling `const` descriptor with the same name,
/// source span, and parent module as each test function. This function performs
/// a single flat scan over all crate items to match descriptors with functions,
/// checking parent equality to ensure true siblings.
#[must_use]
pub fn collect_harness_test_functions(cx: &LateContext<'_>) -> HashSet<hir::HirId> {
    let mut descriptors: Vec<(rustc_span::Symbol, Span, hir::HirId)> = Vec::new();
    let mut candidate_fns: Vec<(hir::HirId, rustc_span::Symbol, Span)> = Vec::new();

    for item_id in cx.tcx.hir_crate_items(()).free_items() {
        let item = cx.tcx.hir_item(item_id);
        let Some(ident) = item.kind.ident() else {
            continue;
        };

        match item.kind {
            hir::ItemKind::Const(..) => {
                descriptors.push((ident.name, item.span, item.hir_id()));
            }
            hir::ItemKind::Fn { .. } => {
                candidate_fns.push((item.hir_id(), ident.name, item.span));
            }
            _ => {}
        }
    }

    candidate_fns
        .into_iter()
        .filter(|(fn_id, name, span)| {
            descriptors.iter().any(|(desc_name, desc_span, desc_id)| {
                desc_id != fn_id && *desc_name == *name && desc_span.source_equal(*span)
            })
        })
        .map(|(hir_id, _, _)| hir_id)
        .collect()
}

/// Collects functions whose rstest companion module contains a harness
/// descriptor.
///
/// The shared [`collect_harness_test_functions`] catches direct const-descriptor
/// siblings. This helper additionally finds functions with a same-named sibling
/// *module* that itself contains a harness descriptor (the pattern rstest case
/// expansions produce). Companions are matched only within the same module scope.
#[must_use]
pub fn collect_rstest_companion_test_functions(cx: &LateContext<'_>) -> HashSet<hir::HirId> {
    let mut marked = HashSet::new();
    let root_mod = cx.tcx.hir_root_module();
    let root_items: Vec<_> = root_mod
        .item_ids
        .iter()
        .map(|id| cx.tcx.hir_item(*id))
        .collect();
    collect_companion_in_group(cx, &root_items, &mut marked);
    marked
}

fn collect_companion_in_group<'tcx>(
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

        if items
            .iter()
            .copied()
            .any(|sibling| has_companion_test_module(cx, item.hir_id(), ident.name, sibling))
        {
            marked.insert(item.hir_id());
        }
    }

    for item in items {
        let hir::ItemKind::Mod(_, module) = item.kind else {
            continue;
        };

        let module_items: Vec<_> = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .collect();
        collect_companion_in_group(cx, &module_items, marked);
    }
}

fn has_companion_test_module<'tcx>(
    cx: &LateContext<'tcx>,
    function_hir_id: hir::HirId,
    function_name: rustc_span::Symbol,
    sibling: &'tcx hir::Item<'tcx>,
) -> bool {
    sibling.hir_id() != function_hir_id
        && sibling
            .kind
            .ident()
            .is_some_and(|ident| ident.name == function_name)
        && matches!(sibling.kind, hir::ItemKind::Mod(..))
        && module_has_harness_descriptor(cx, sibling)
}

fn module_has_harness_descriptor<'tcx>(
    cx: &LateContext<'tcx>,
    module_item: &'tcx hir::Item<'tcx>,
) -> bool {
    let hir::ItemKind::Mod(_, module) = module_item.kind else {
        return false;
    };

    let items: Vec<_> = module
        .item_ids
        .iter()
        .map(|item_id| cx.tcx.hir_item(*item_id))
        .collect();

    items
        .iter()
        .copied()
        .filter(|item| matches!(item.kind, hir::ItemKind::Fn { .. }))
        .any(|fn_item| {
            let Some((fn_id, fn_name, _, fn_span)) =
                item_components(cx, fn_item, |k| matches!(k, hir::ItemKind::Fn { .. }))
            else {
                return false;
            };
            items.iter().copied().any(|sibling| {
                item_components(cx, sibling, |k| matches!(k, hir::ItemKind::Const(..))).is_some_and(
                    |(s_id, s_name, _, s_span)| {
                        s_id != fn_id && s_name == fn_name && s_span.source_equal(fn_span)
                    },
                )
            })
        })
}

mod tests;
