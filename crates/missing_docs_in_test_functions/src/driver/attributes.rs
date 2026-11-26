//! Maps rustc attributes onto the documentation-relevant shapes.
//!
//! Classification is by attribute shape, not by resolved value, because before
//! expansion a `#[doc = include_str!("...")]` value is an unexpanded macro
//! call. Asking for the attribute's string value would report that function as
//! undocumented and then diagnose it, which the contract forbids.

use rustc_ast::{
    ast::{AttrArgs, AttrItem, AttrKind, Attribute},
    token::TokenKind,
    tokenstream::TokenTree,
};
use rustc_span::sym;

use crate::policy::DocAttributeShape;

/// Classifies one function attribute.
pub(super) fn doc_attribute_shape(attribute: &Attribute) -> DocAttributeShape {
    match &attribute.kind {
        AttrKind::DocComment(..) => DocAttributeShape::DocComment,
        AttrKind::Normal(normal) => normal_attribute_shape(&normal.item),
    }
}

/// Classifies a `#[...]` attribute by its path and argument shape.
fn normal_attribute_shape(item: &AttrItem) -> DocAttributeShape {
    if item.path == sym::doc {
        return doc_attribute_args_shape(item.args.unparsed_ref());
    }
    if item.path == sym::cfg_attr && cfg_attr_args_carry_doc(item) {
        return DocAttributeShape::CfgAttrDoc;
    }
    DocAttributeShape::Other
}

/// Classifies a `doc` attribute by the shape of its arguments.
const fn doc_attribute_args_shape(args: Option<&AttrArgs>) -> DocAttributeShape {
    match args {
        // `#[doc = "..."]` and `#[doc = include_str!("...")]`; the value is an
        // unexpanded expression, so its form is never inspected.
        Some(AttrArgs::Eq { .. }) => DocAttributeShape::DocNameValue,
        // `#[doc(hidden)]`, `#[doc(alias = "...")]`, and the bare `#[doc]`.
        Some(AttrArgs::Delimited(..) | AttrArgs::Empty) => DocAttributeShape::DocList,
        // An early-parsed `cfg_attr` is produced during expansion, after this
        // pass has run; treat anything unexpected as non-documentation rather
        // than guessing.
        None => DocAttributeShape::Other,
    }
}

/// Reports whether a `cfg_attr` argument list carries a `doc = ...` entry.
///
/// The tokens are inspected directly rather than through `meta_item_list()`.
/// That parser accepts only a literal after `=`, so it returns nothing for
/// `#[cfg_attr(test, doc = include_str!("docs.txt"))]` and the function would
/// be diagnosed despite being documented.
///
/// The whole path must be `doc`: matching a bare `doc` identifier would also
/// accept `clippy::doc`, whose final segment is `doc`, and silence a genuine
/// diagnostic. Only a single-segment path is documentation.
///
/// Nested `cfg_attr` payloads are scanned too. Rust expands
/// `#[cfg_attr(test, cfg_attr(test, doc = "..."))]` to a `doc` attribute, so a
/// function documented only through the inner attribute is still documented.
fn cfg_attr_args_carry_doc(item: &AttrItem) -> bool {
    meta_entries(item).is_some_and(|entries| args_carry_doc_after_predicate(&entries))
}

/// Reports whether any entry after a `cfg_attr` list's leading predicate is a
/// documentation entry.
///
/// Shared by the outer attribute and by each nested payload, so the two cannot
/// drift in how they decide where the predicate ends.
fn args_carry_doc_after_predicate(entries: &[&TokenTree]) -> bool {
    let Some(predicate_end_index) = first_comma(entries) else {
        return false;
    };
    let after_predicate = predicate_end_index + 1;
    (after_predicate..entries.len()).any(|index| is_documentation_entry(entries, index))
}

/// Reports whether the entry at `index` is a doc assignment or a nested
/// `cfg_attr` that carries one.
fn is_documentation_entry(entries: &[&TokenTree], index: usize) -> bool {
    is_doc_assignment(entries, index) || nested_cfg_attr_carries_doc(entries, index)
}

/// Returns the attribute's argument tokens as a flat entry list, if it has any.
fn meta_entries(item: &AttrItem) -> Option<Vec<&TokenTree>> {
    let AttrArgs::Delimited(args) = item.args.unparsed_ref()? else {
        return None;
    };
    Some(args.tokens.iter().collect())
}

/// Returns the kind of a token tree, or `None` when it is a delimited group.
///
/// Every predicate in this module is a test on one token's kind, so unwrapping
/// the token once here keeps the destructuring out of each of them.
const fn token_kind(tree: &TokenTree) -> Option<&TokenKind> {
    match tree {
        TokenTree::Token(token, _) => Some(&token.kind),
        TokenTree::Delimited(..) => None,
    }
}

/// Returns the index of the first top-level comma in `entries`.
///
/// Commas inside a parenthesized predicate arrive inside a `Delimited` tree,
/// so they are not at this level and cannot be mistaken for this separator.
fn first_comma(entries: &[&TokenTree]) -> Option<usize> {
    entries
        .iter()
        .position(|tree| matches!(token_kind(tree), Some(TokenKind::Comma)))
}

/// Reports whether the entry at `index` starts a bare `doc = ...` assignment.
fn is_doc_assignment(entries: &[&TokenTree], index: usize) -> bool {
    is_bare_doc_path(entries, index) && entries.get(index + 1).is_some_and(|next| is_eq_sign(next))
}

/// Reports whether the path ending at `index` is the single segment `doc`.
///
/// `doc = ...` and `clippy::doc = ...` both end in a `doc` identifier followed
/// by `=`, so the identifier alone cannot decide. A single-segment path is
/// either the first entry after the predicate or is preceded by a comma; a
/// qualified path is preceded by a `::` tree.
///
/// The raw spelling `r#doc` names the same attribute, so it counts. Only the
/// path separator distinguishes a qualified path; the raw prefix does not.
fn is_bare_doc_path(entries: &[&TokenTree], index: usize) -> bool {
    is_bare_path(entries, index, sym::doc)
}

/// Reports whether the path ending at `index` is the single segment `name`.
///
/// A qualified path such as `clippy::doc` or `clippy::cfg_attr` ends in the
/// same identifier but names a different, tool-scoped attribute. Those are
/// inert metadata, not documentation and not a conditional attribute, so the
/// preceding `::` tree excludes them. The raw spelling `r#doc` names the same
/// attribute, so the raw prefix is irrelevant.
fn is_bare_path(entries: &[&TokenTree], index: usize, name: rustc_span::Symbol) -> bool {
    let Some(tree) = entries.get(index) else {
        return false;
    };
    if !matches!(token_kind(tree), Some(TokenKind::Ident(found, _)) if *found == name) {
        return false;
    }
    !entries
        .get(index.wrapping_sub(1))
        .is_some_and(|previous| matches!(token_kind(previous), Some(TokenKind::PathSep)))
}

/// Reports whether the entry at `index` names `cfg_attr` and its parenthesized
/// payload carries a doc assignment.
///
/// A `cfg_attr` in a token stream is two entries: the identifier and the
/// delimited argument list. Nested attributes nest through that payload, so the
/// scan recurses once per level. A qualified `clippy::cfg_attr` is inert tool
/// metadata, so only a single-segment path counts.
fn nested_cfg_attr_carries_doc(entries: &[&TokenTree], index: usize) -> bool {
    if !is_bare_path(entries, index, sym::cfg_attr) {
        return false;
    }
    let Some(TokenTree::Delimited(_, _, _, inner)) = entries.get(index + 1) else {
        return false;
    };
    let payload: Vec<&TokenTree> = inner.iter().collect();
    args_carry_doc_after_predicate(&payload)
}

/// Reports whether `tree` is the `=` sign of a name-value meta item.
const fn is_eq_sign(tree: &TokenTree) -> bool { matches!(token_kind(tree), Some(TokenKind::Eq)) }
