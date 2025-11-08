//! Convert HIR ancestors into simplified context entries and detect test-only
//! guards (for example, `cfg(test)`), supporting the lint's context
//! summarisation.

use common::{
    Attribute, AttributeKind, AttributePath, ContextEntry, ContextKind, in_test_like_context_with,
};
use rustc_ast::AttrStyle;
use rustc_ast::ast::{MetaItem, MetaItemInner, Path};
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

    let mut ancestors: Vec<_> = cx.tcx.hir_parent_iter(hir_id).collect();
    ancestors.reverse();

    for (ancestor_id, node) in ancestors {
        let attrs = cx.tcx.hir_attrs(ancestor_id);
        if attrs.iter().any(is_cfg_test_attribute) {
            has_cfg_test = true;
        }

        if let Some(entry) = context_entry_for(cx, node, attrs) {
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

fn context_entry_for<'tcx>(
    cx: &LateContext<'tcx>,
    node: Node<'tcx>,
    attrs: &[hir::Attribute],
) -> Option<ContextEntry> {
    match node {
        Node::Item(item) => match &item.kind {
            hir::ItemKind::Fn { .. } => {
                let name = cx
                    .tcx
                    .opt_item_ident(item.owner_id.def_id)
                    .map(|ident| ident.name.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                Some(ContextEntry::function(name, convert_attributes(attrs)))
            }
            hir::ItemKind::Mod(..) => {
                let name = cx
                    .tcx
                    .opt_item_ident(item.owner_id.def_id)
                    .map(|ident| ident.name.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                Some(ContextEntry::new(
                    name,
                    ContextKind::Module,
                    convert_attributes(attrs),
                ))
            }
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
    attrs
        .iter()
        .filter(|attr| !is_cfg_test_attribute(attr))
        .map(convert_attribute)
        .collect()
}

fn convert_attribute(attr: &hir::Attribute) -> Attribute {
    let kind = match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    };
    let path = attribute_path(attr);

    Attribute::new(path, kind)
}

fn attribute_path(attr: &hir::Attribute) -> AttributePath {
    if attr.doc_str().is_some() {
        AttributePath::from("doc")
    } else {
        let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
        match names.next() {
            Some(first) => AttributePath::new(std::iter::once(first).chain(names)),
            None => AttributePath::from("unknown"),
        }
    }
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Parsed(kind) => match kind {
            hir::attrs::AttributeKind::DocComment { style, .. } => *style,
            _ => AttrStyle::Outer,
        },
        hir::Attribute::Unparsed(item) => item.style,
    }
}

#[cfg(test)]
mod tests {
    use super::meta_contains_test_cfg;
    use rustc_ast::ast::{MetaItem, MetaItemInner, MetaItemKind};
    use rustc_ast::{Path, PathSegment};
    use rustc_span::{create_default_session_globals_then, symbol::Ident, DUMMY_SP};

    fn path_from_segments(segments: &[&str]) -> Path {
        let path_segments = segments
            .iter()
            .map(|segment| PathSegment::from_ident(Ident::from_str(segment)))
            .collect::<Vec<_>>();

        Path {
            span: DUMMY_SP,
            segments: path_segments.into(),
            tokens: None,
        }
    }

    fn meta_word(segments: &[&str]) -> MetaItem {
        MetaItem {
            path: path_from_segments(segments),
            kind: MetaItemKind::Word,
            span: DUMMY_SP,
            unsafety: rustc_ast::ast::Safety::Default,
        }
    }

    fn meta_list(segments: &[&str], children: Vec<MetaItemInner>) -> MetaItem {
        MetaItem {
            path: path_from_segments(segments),
            kind: MetaItemKind::List(children.into()),
            span: DUMMY_SP,
            unsafety: rustc_ast::ast::Safety::Default,
        }
    }

    fn meta_inner(meta: MetaItem) -> MetaItemInner {
        MetaItemInner::MetaItem(meta)
    }

    #[test]
    fn meta_contains_test_cfg_cases() {
        create_default_session_globals_then(|| {
            let cases = [
                (
                    meta_list(
                        &["cfg"],
                        vec![meta_inner(meta_list(
                            &["any"],
                            vec![
                                meta_inner(meta_word(&["test"])),
                                meta_inner(meta_word(&["doctest"])),
                            ],
                        ))],
                    ),
                    true,
                ),
                (
                    meta_list(
                        &["cfg"],
                        vec![meta_inner(meta_list(
                            &["all"],
                            vec![
                                meta_inner(meta_word(&["test"])),
                                meta_inner(meta_word(&["unix"])),
                            ],
                        ))],
                    ),
                    true,
                ),
                (
                    meta_list(
                        &["cfg"],
                        vec![meta_inner(meta_list(
                            &["not"],
                            vec![meta_inner(meta_word(&["test"]))],
                        ))],
                    ),
                    false,
                ),
                (
                    meta_list(
                        &["cfg_attr"],
                        vec![
                            meta_inner(meta_word(&["test"])),
                            meta_inner(meta_list(&["cfg"], vec![meta_inner(meta_word(&["test"]))])),
                        ],
                    ),
                    true,
                ),
                (
                    meta_list(
                        &["cfg_attr"],
                        vec![
                            meta_inner(meta_word(&["test"])),
                            meta_inner(meta_list(
                                &["allow"],
                                vec![meta_inner(meta_word(&["dead_code"]))],
                            )),
                        ],
                    ),
                    false,
                ),
            ];

            for (meta, expected) in cases {
                assert_eq!(meta_contains_test_cfg(&meta), expected);
            }
        });
    }
}

fn is_cfg_test_attribute(attr: &hir::Attribute) -> bool {
    if attr_is_path(attr, sym::cfg) {
        return attr
            .meta_item_list()
            .map(|items| items.iter().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if !attr_is_path(attr, sym::cfg_attr) {
        return false;
    }

    let Some(items) = attr.meta_item_list() else {
        return false;
    };
    let mut iter = items.iter();
    let Some(condition) = iter.next() else {
        return false;
    };

    if !meta_item_inner_contains_test(condition) {
        return false;
    }

    iter.any(|item| match item {
        MetaItemInner::MetaItem(inner) => meta_contains_test_cfg(inner),
        MetaItemInner::Lit(_) => false,
    })
}

fn attr_is_path(attr: &hir::Attribute, symbol: rustc_span::Symbol) -> bool {
    attr.ident_path()
        .map(|segments| segments.len() == 1 && segments[0].name == symbol)
        .unwrap_or(false)
}

fn meta_item_inner_contains_test(item: &MetaItemInner) -> bool {
    meta_item_inner_contains_test_with_polarity(item, true)
}

fn meta_item_inner_contains_test_with_polarity(item: &MetaItemInner, is_positive: bool) -> bool {
    match item {
        MetaItemInner::MetaItem(meta) => meta_contains_test_with_polarity(meta, is_positive),
        MetaItemInner::Lit(_) => false,
    }
}

fn meta_contains_test(meta: &MetaItem) -> bool {
    meta_contains_test_with_polarity(meta, true)
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
                    .any(|item| meta_item_inner_contains_test_with_polarity(item, !is_positive))
            })
            .unwrap_or(false);
    }

    meta.meta_item_list()
        .map(|items| {
            items
                .iter()
                .any(|item| meta_item_inner_contains_test_with_polarity(item, is_positive))
        })
        .unwrap_or(false)
}

fn meta_contains_test_cfg(meta: &MetaItem) -> bool {
    if path_is_ident(&meta.path, sym::cfg) {
        return meta
            .meta_item_list()
            .map(|items| items.iter().any(meta_item_inner_contains_test))
            .unwrap_or(false);
    }

    if !path_is_ident(&meta.path, sym::cfg_attr) {
        return false;
    }

    let Some(items) = meta.meta_item_list() else {
        return false;
    };
    let mut iter = items.iter();
    let Some(condition) = iter.next() else {
        return false;
    };

    if !meta_item_inner_contains_test(condition) {
        return false;
    }

    iter.any(|item| match item {
        MetaItemInner::MetaItem(inner) => meta_contains_test_cfg(inner),
        MetaItemInner::Lit(_) => false,
    })
}

fn path_is_ident(path: &Path, symbol: rustc_span::Symbol) -> bool {
    path.segments.len() == 1 && path.segments[0].ident.name == symbol
}
