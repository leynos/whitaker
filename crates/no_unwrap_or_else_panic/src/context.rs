//! Context discovery for `unwrap_or_else` sites.
//!
//! Converts HIR ancestors into simplified context entries so the lint can
//! detect test-like scopes, doctest guards, and `main` functions.

use common::{Attribute, AttributeKind, AttributePath, ContextEntry, ContextKind};

/// Summary of the surrounding context for a HIR node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContextSummary {
    pub(crate) is_test: bool,
    pub(crate) in_main: bool,
}

#[cfg(feature = "dylint-driver")]
use rustc_ast::ast::{MetaItem, MetaItemInner};
#[cfg(feature = "dylint-driver")]
use rustc_ast::{AttrStyle, Path as AstPath};
#[cfg(feature = "dylint-driver")]
use rustc_hir as hir;
#[cfg(feature = "dylint-driver")]
use rustc_hir::Node;
#[cfg(feature = "dylint-driver")]
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
#[cfg(feature = "dylint-driver")]
use rustc_lint::LateContext;
#[cfg(feature = "dylint-driver")]
use rustc_span::sym;

/// Summarise the context for a given HIR node.
#[cfg(feature = "dylint-driver")]
pub(crate) fn summarise_context<'tcx>(
    cx: &LateContext<'tcx>,
    hir_id: hir::HirId,
) -> ContextSummary {
    let mut entries = Vec::new();
    let mut has_cfg_test = false;

    let mut ancestors: Vec<_> = cx.tcx.hir_parent_iter(hir_id).collect();
    ancestors.reverse();

    for (ancestor_id, node) in ancestors {
        let attrs = cx.tcx.hir_attrs(ancestor_id);
        if attrs.iter().any(is_cfg_test_attribute) {
            has_cfg_test = true;
        }

        if let Some(entry) = context_entry_for(node, attrs) {
            entries.push(entry);
        }
    }

    let is_test = has_cfg_test || common::in_test_like_context(entries.as_slice());
    let in_main = common::is_in_main_fn(entries.as_slice());

    ContextSummary { is_test, in_main }
}

#[cfg(feature = "dylint-driver")]
fn context_entry_for(node: Node<'_>, attrs: &[hir::Attribute]) -> Option<ContextEntry> {
    match node {
        Node::Item(item) => match &item.kind {
            hir::ItemKind::Fn { .. } => {
                item_name(item).map(|name| ContextEntry::function(name, convert_attributes(attrs)))
            }
            hir::ItemKind::Mod { .. } => item_name(item).map(|name| {
                ContextEntry::new(name, ContextKind::Module, convert_attributes(attrs))
            }),
            hir::ItemKind::Impl(..) => Some(ContextEntry::new(
                "impl".to_string(),
                ContextKind::Impl,
                convert_attributes(attrs),
            )),
            _ => None,
        },
        Node::ImplItem(item) => match item.kind {
            hir::ImplItemKind::Fn(..) => Some(ContextEntry::function(
                item.ident.name.to_string(),
                convert_attributes(attrs),
            )),
            _ => None,
        },
        Node::TraitItem(item) => match item.kind {
            hir::TraitItemKind::Fn(..) => Some(ContextEntry::function(
                item.ident.name.to_string(),
                convert_attributes(attrs),
            )),
            _ => None,
        },
        Node::Block(_) => Some(ContextEntry::new(
            "block".to_string(),
            ContextKind::Block,
            convert_attributes(attrs),
        )),
        _ => None,
    }
}

#[cfg(feature = "dylint-driver")]
fn convert_attributes(attrs: &[hir::Attribute]) -> Vec<Attribute> {
    attrs.iter().map(convert_attribute).collect()
}

#[cfg(feature = "dylint-driver")]
fn convert_attribute(attr: &hir::Attribute) -> Attribute {
    let kind = match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    };
    let path = if attr.doc_str().is_some() {
        AttributePath::from("doc")
    } else {
        let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
        match names.next() {
            Some(first) => AttributePath::new(std::iter::once(first).chain(names)),
            None => AttributePath::from("unknown"),
        }
    };

    Attribute::new(path, kind)
}

#[cfg(feature = "dylint-driver")]
fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Unparsed(item) => item.style,
        hir::Attribute::Parsed(HirAttributeKind::DocComment { style, .. }) => *style,
        _ => AttrStyle::Outer,
    }
}

#[cfg(feature = "dylint-driver")]
fn item_name(item: &hir::Item<'_>) -> Option<String> {
    item.kind.ident().map(|ident| ident.name.to_string())
}

#[cfg(feature = "dylint-driver")]
fn is_cfg_test_attribute(attr: &hir::Attribute) -> bool {
    let path = attr.path();
    if path.len() != 1 {
        return false;
    }

    if path[0] == sym::cfg {
        return attr
            .meta_item_list()
            .map(|items| items.iter().cloned().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if path[0] != sym::cfg_attr {
        return false;
    }

    attr.meta_item_list()
        .map(check_cfg_attr_for_test)
        .unwrap_or(false)
}

#[cfg(feature = "dylint-driver")]
fn check_cfg_attr_for_test<I>(items: I) -> bool
where
    I: IntoIterator<Item = MetaItemInner>,
{
    let mut iter = items.into_iter();
    let Some(condition) = iter.next() else {
        return false;
    };

    if !meta_item_inner_contains_test(condition) {
        return false;
    }

    iter.any(|item| match item {
        MetaItemInner::MetaItem(inner) => meta_contains_test_cfg(&inner),
        MetaItemInner::Lit(_) => false,
    })
}

#[cfg(feature = "dylint-driver")]
fn meta_item_inner_contains_test(item: MetaItemInner) -> bool {
    meta_item_inner_contains_test_with_polarity(item, true)
}

#[cfg(feature = "dylint-driver")]
fn meta_item_inner_contains_test_with_polarity(item: MetaItemInner, is_positive: bool) -> bool {
    match item {
        MetaItemInner::MetaItem(meta) => meta_contains_test_with_polarity(&meta, is_positive),
        MetaItemInner::Lit(_) => false,
    }
}

#[cfg(feature = "dylint-driver")]
fn meta_contains_test_with_polarity(meta: &MetaItem, is_positive: bool) -> bool {
    if path_is_ident(&meta.path, sym::test) || path_is_ident(&meta.path, sym::doctest) {
        return is_positive;
    }

    if path_is_ident(&meta.path, sym::not) {
        return meta
            .meta_item_list()
            .map(|items| {
                items
                    .iter()
                    .cloned()
                    .any(|item| meta_item_inner_contains_test_with_polarity(item, !is_positive))
            })
            .unwrap_or(false);
    }

    meta.meta_item_list()
        .map(|items| items.iter().cloned().any(meta_item_inner_contains_test))
        .unwrap_or(false)
}

#[cfg(feature = "dylint-driver")]
fn meta_contains_test_cfg(meta: &MetaItem) -> bool {
    meta.meta_item_list()
        .map(|items| items.iter().cloned().any(meta_item_inner_contains_test))
        .unwrap_or(false)
}

#[cfg(feature = "dylint-driver")]
fn path_is_ident(path: &AstPath, ident: rustc_span::Symbol) -> bool {
    path.segments.len() == 1 && path.segments[0].ident.name == ident
}
