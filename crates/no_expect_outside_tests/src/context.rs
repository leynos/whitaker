use common::{
    Attribute, AttributeKind, AttributePath, ContextEntry, ContextKind, in_test_like_context_with,
};
use rustc_ast::AttrStyle;
use rustc_ast::ast::{MetaItem, MetaItemInner};
use rustc_hir as hir;
use rustc_hir::Node;
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

    for (ancestor_id, node) in cx.tcx.hir_parent_iter(hir_id) {
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
    let function_name = entries.iter().find_map(|entry| {
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
            hir::ItemKind::Fn { ident, .. } => Some(ContextEntry::function(
                ident.name.to_string(),
                convert_attributes(attrs),
            )),
            hir::ItemKind::Mod(ident, ..) => Some(ContextEntry::new(
                ident.name.to_string(),
                ContextKind::Module,
                convert_attributes(attrs),
            )),
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
    let kind = match attr.style() {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    };
    let path = if attr.doc_str().is_some() {
        AttributePath::from("doc")
    } else if let Some(segments) = attr.ident_path() {
        let names = segments.into_iter().map(|ident| ident.name.to_string());
        AttributePath::new(names)
    } else if let Some(name) = attr.name() {
        AttributePath::from(name.to_string())
    } else {
        AttributePath::from("unknown")
    };

    Attribute::new(path, kind)
}

fn is_cfg_test_attribute(attr: &hir::Attribute) -> bool {
    if attr.has_name(sym::cfg) {
        return attr
            .meta_item_list()
            .map(|items| items.into_iter().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if !attr.has_name(sym::cfg_attr) {
        return false;
    }

    let Some(mut items) = attr.meta_item_list() else {
        return false;
    };
    let mut iter = items.into_iter();
    let Some(condition) = iter.next() else {
        return false;
    };

    if !meta_item_inner_contains_test(condition) {
        return false;
    }

    iter.any(|item| match item {
        MetaItemInner::MetaItem(meta) => meta_contains_test_cfg(&meta),
        MetaItemInner::Lit(_) => false,
    })
}

fn meta_item_inner_contains_test(item: MetaItemInner) -> bool {
    match item {
        MetaItemInner::MetaItem(meta) => meta_contains_test(&meta),
        MetaItemInner::Lit(_) => false,
    }
}

fn meta_contains_test(meta: &MetaItem) -> bool {
    if meta.path.is_ident(sym::test) {
        return true;
    }

    meta.meta_item_list()
        .map(|items| items.into_iter().any(meta_item_inner_contains_test))
        .unwrap_or(false)
}

fn meta_contains_test_cfg(meta: &MetaItem) -> bool {
    if meta.path.is_ident(sym::cfg) {
        return meta
            .meta_item_list()
            .map(|items| items.into_iter().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if !meta.path.is_ident(sym::cfg_attr) {
        return false;
    }

    let Some(mut items) = meta.meta_item_list() else {
        return false;
    };
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
