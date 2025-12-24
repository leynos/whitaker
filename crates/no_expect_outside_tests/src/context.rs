//! Convert HIR ancestors into simplified context entries and detect test-only
//! guards (for example, `cfg(test)`), supporting the lint's context
//! summarisation.

use common::{
    Attribute, AttributeKind, AttributePath, ContextEntry, ContextKind, in_test_like_context_with,
};
use rustc_ast::AttrStyle;
use rustc_ast::ast::{MetaItem, MetaItemInner};
use rustc_hir as hir;
use rustc_hir::Node;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::LateContext;
use rustc_span::sym;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextSummary {
    pub(crate) is_test: bool,
    pub(crate) function_name: Option<String>,
}

pub(crate) fn collect_context<'tcx>(
    cx: &LateContext<'tcx>,
    hir_id: hir::HirId,
    _additional_test_attributes: &[AttributePath],
) -> (Vec<ContextEntry>, bool) {
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

    (entries, has_cfg_test)
}

pub(crate) fn summarise_context(
    entries: &[ContextEntry],
    has_cfg_test: bool,
    additional_test_attributes: &[AttributePath],
) -> ContextSummary {
    let is_test = has_cfg_test || in_test_like_context_with(entries, additional_test_attributes);
    let function_name = entries.iter().rev().find_map(|entry| {
        entry
            .kind()
            .matches_function()
            .then(|| entry.name().to_string())
    });

    ContextSummary {
        is_test,
        function_name,
    }
}

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

fn convert_attributes(attrs: &[hir::Attribute]) -> Vec<Attribute> {
    attrs.iter().map(convert_attribute).collect()
}

fn convert_attribute(attr: &hir::Attribute) -> Attribute {
    let kind = match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    };
    let path = if attr.doc_str().is_some() {
        AttributePath::from("doc")
    } else {
        // Parsed attributes (like #[must_use]) don't have an accessible path;
        // calling path() on them would panic.
        let hir::Attribute::Unparsed(_) = attr else {
            return Attribute::new(AttributePath::from("parsed"), kind);
        };
        let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
        match names.next() {
            Some(first) => AttributePath::new(std::iter::once(first).chain(names)),
            None => AttributePath::from("unknown"),
        }
    };

    Attribute::new(path, kind)
}

/// Check if a cfg_attr has a test condition and contains nested cfg(test).
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

fn is_cfg_test_attribute(attr: &hir::Attribute) -> bool {
    // Parsed attributes (like #[must_use]) are not cfg-related; skip them to
    // avoid panics when calling path() on arbitrary parsed attributes.
    let hir::Attribute::Unparsed(_) = attr else {
        return false;
    };

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

fn meta_item_inner_contains_test(item: MetaItemInner) -> bool {
    meta_item_inner_contains_test_with_polarity(item, true)
}

fn meta_item_inner_contains_test_with_polarity(item: MetaItemInner, is_positive: bool) -> bool {
    match item {
        MetaItemInner::MetaItem(meta) => meta_contains_test_with_polarity(&meta, is_positive),
        MetaItemInner::Lit(_) => false,
    }
}

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
        .map(|items| {
            items
                .iter()
                .cloned()
                .any(|item| meta_item_inner_contains_test_with_polarity(item, is_positive))
        })
        .unwrap_or(false)
}

fn meta_contains_test_cfg(meta: &MetaItem) -> bool {
    if path_is_ident(&meta.path, sym::cfg) {
        return meta
            .meta_item_list()
            .map(|items| items.iter().cloned().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if !path_is_ident(&meta.path, sym::cfg_attr) {
        return false;
    }

    meta.meta_item_list()
        .map(|items| check_cfg_attr_for_test(items.iter().cloned()))
        .unwrap_or(false)
}

fn item_name(item: &hir::Item<'_>) -> Option<String> {
    item.kind.ident().map(|ident| ident.name.to_string())
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Unparsed(item) => item.style,
        hir::Attribute::Parsed(HirAttributeKind::DocComment { style, .. }) => *style,
        _ => AttrStyle::Outer,
    }
}

fn path_is_ident(path: &rustc_ast::Path, symbol: rustc_span::Symbol) -> bool {
    path.segments.len() == 1 && path.segments[0].ident.name == symbol
}

#[cfg(test)]
mod tests;
