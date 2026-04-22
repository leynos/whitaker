//! Convert HIR ancestors into simplified context entries and detect test-only
//! guards (for example, `cfg(test)`), supporting the lint's context
//! summarisation.

use rustc_ast::AttrStyle;
use rustc_ast::ast::{MetaItem, MetaItemInner};
use rustc_hir as hir;
use rustc_hir::Node;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::LateContext;
use rustc_span::sym;
use whitaker::hir::has_test_like_hir_attributes;
use whitaker_common::{
    Attribute, AttributeKind, AttributePath, ContextEntry, ContextKind,
    PARSED_ATTRIBUTE_PLACEHOLDER, in_test_like_context_with,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextSummary {
    pub(crate) is_test: bool,
    pub(crate) function_name: Option<String>,
}

/// Collects simplified context entries for the ancestors of a HIR node.
///
/// Walks the ancestor chain for `hir_id`, records any context-bearing nodes as
/// `ContextEntry` values, and tracks whether any ancestor establishes test
/// context through `cfg(test)` or a recognized test attribute.
///
/// # Parameters
///
/// - `cx`: Lint context used to walk the HIR and inspect ancestor attributes.
/// - `hir_id`: The HIR node whose ancestor chain should be summarized.
/// - `additional_test_attributes`: Extra user-configured attribute paths that
///   should be treated as test markers alongside Whitaker's built-in list.
///
/// # Returns
///
/// Returns `(entries, has_test_context_ancestry)`, where `entries` is the
/// ordered list of simplified ancestor contexts and the boolean records whether
/// any ancestor already established test-only ancestry.
///
/// # Examples
///
/// ```ignore
/// let (entries, has_test_context_ancestry) =
///     collect_context(cx, expr.hir_id, additional_test_attributes);
/// assert!(!entries.is_empty() || !has_test_context_ancestry);
/// ```
pub(crate) fn collect_context<'tcx>(
    cx: &LateContext<'tcx>,
    hir_id: hir::HirId,
    additional_test_attributes: &[AttributePath],
) -> (Vec<ContextEntry>, bool) {
    let mut entries = Vec::new();
    let mut has_test_context_ancestry = false;

    let mut ancestors: Vec<_> = cx.tcx.hir_parent_iter(hir_id).collect();
    ancestors.reverse();

    for (ancestor_id, node) in ancestors {
        let attrs = cx.tcx.hir_attrs(ancestor_id);
        has_test_context_ancestry = has_test_ancestry(
            has_test_context_ancestry,
            attrs,
            matches!(node, Node::Item(item) if matches!(item.kind, hir::ItemKind::Fn { .. })),
            additional_test_attributes,
        );

        if let Some(entry) = context_entry_for(node, attrs) {
            entries.push(entry);
        }
    }

    (entries, has_test_context_ancestry)
}

fn has_test_ancestry(
    has_test_context_ancestry: bool,
    attrs: &[hir::Attribute],
    is_function_item: bool,
    additional_test_attributes: &[AttributePath],
) -> bool {
    has_test_context_ancestry
        || attrs.iter().any(is_cfg_test_attribute)
        || (is_function_item && has_test_like_hir_attributes(attrs, additional_test_attributes))
}

/// Summarizes collected ancestor context into the lint's final test decision.
///
/// Combines the pre-computed ancestor entries with the carried
/// `has_test_context_ancestry` flag so the lint can determine whether the
/// current call site should be treated as test-only code.
///
/// # Parameters
///
/// - `entries`: Simplified ancestor contexts produced by `collect_context`.
/// - `has_test_context_ancestry`: Whether any ancestor already established
///   test-only ancestry via propagation, `cfg(test)`, or a recognized
///   test-marker attribute.
/// - `additional_test_attributes`: Extra user-configured attribute paths that
///   should be considered test markers during the final summary check.
///
/// # Returns
///
/// Returns a `ContextSummary` describing the derived test-context status and
/// the innermost enclosing function name, if one was found.
///
/// # Examples
///
/// ```ignore
/// let summary = summarise_context(
///     &entries,
///     has_test_context_ancestry,
///     additional_test_attributes,
/// );
/// if summary.is_test {
///     // `.expect()` is allowed in this context.
/// }
/// ```
pub(crate) fn summarise_context(
    entries: &[ContextEntry],
    has_test_context_ancestry: bool,
    additional_test_attributes: &[AttributePath],
) -> ContextSummary {
    let is_test =
        has_test_context_ancestry || in_test_like_context_with(entries, additional_test_attributes);
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
            return Attribute::new(AttributePath::from(PARSED_ATTRIBUTE_PLACEHOLDER), kind);
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

/// Returns whether a HIR attribute enables `cfg(test)` semantics.
///
/// Recognizes both direct `cfg(test)` attributes and `cfg_attr(.., cfg(test))`
/// forms so callers can treat such ancestors as test-only context.
///
/// # Parameters
///
/// - `attr`: The HIR attribute to inspect.
///
/// # Returns
///
/// Returns `true` when the attribute is a `cfg(test)`-style marker and `false`
/// otherwise.
///
/// # Examples
///
/// ```ignore
/// if attrs.iter().any(is_cfg_test_attribute) {
///     // The enclosing item participates in test-only compilation.
/// }
/// ```
pub(crate) fn is_cfg_test_attribute(attr: &hir::Attribute) -> bool {
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
