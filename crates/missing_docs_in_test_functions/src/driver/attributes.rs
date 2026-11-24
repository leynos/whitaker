//! Maps rustc attributes onto the documentation-relevant shapes.
//!
//! Classification is by attribute shape, not by resolved value, because before
//! expansion a `#[doc = include_str!("...")]` value is an unexpanded macro
//! call. Asking for the attribute's string value would report that function as
//! undocumented and then diagnose it, which the contract forbids.

use rustc_ast::{
    ast::{AttrArgs, AttrItem, AttrKind, Attribute},
    token::{IdentIsRaw, Token, TokenKind},
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
fn cfg_attr_args_carry_doc(item: &AttrItem) -> bool {
    let Some(entries) = meta_entries(item) else {
        return false;
    };
    let Some(predicate_end_index) = first_comma(&entries) else {
        return false;
    };
    let after_predicate = predicate_end_index + 1;
    (after_predicate..entries.len()).any(|index| is_doc_assignment(&entries, index))
}

/// Returns the attribute's argument tokens as a flat entry list, if it has any.
fn meta_entries(item: &AttrItem) -> Option<Vec<&TokenTree>> {
    let AttrArgs::Delimited(args) = item.args.unparsed_ref()? else {
        return None;
    };
    Some(args.tokens.iter().collect())
}

/// Returns the index of the first top-level comma in `entries`.
///
/// Commas inside a parenthesized predicate arrive inside a `Delimited` tree,
/// so they are not at this level and cannot be mistaken for this separator.
fn first_comma(entries: &[&TokenTree]) -> Option<usize> {
    entries.iter().position(|tree| {
        matches!(
            tree,
            TokenTree::Token(
                Token {
                    kind: TokenKind::Comma,
                    ..
                },
                _
            )
        )
    })
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
fn is_bare_doc_path(entries: &[&TokenTree], index: usize) -> bool {
    let Some(tree) = entries.get(index) else {
        return false;
    };
    let TokenTree::Token(
        Token {
            kind: TokenKind::Ident(name, raw),
            ..
        },
        _,
    ) = tree
    else {
        return false;
    };
    if *name != sym::doc || *raw == IdentIsRaw::Yes {
        return false;
    }
    !matches!(
        entries.get(index.wrapping_sub(1)),
        Some(TokenTree::Token(
            Token {
                kind: TokenKind::PathSep,
                ..
            },
            _
        ))
    )
}

/// Reports whether `tree` is the `=` sign of a name-value meta item.
const fn is_eq_sign(tree: &TokenTree) -> bool {
    matches!(
        tree,
        TokenTree::Token(
            Token {
                kind: TokenKind::Eq,
                ..
            },
            _,
        )
    )
}
