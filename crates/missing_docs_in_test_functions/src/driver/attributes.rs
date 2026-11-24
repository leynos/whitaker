//! Maps rustc attributes onto the documentation-relevant shapes.
//!
//! Classification is by attribute shape, not by resolved value, because before
//! expansion a `#[doc = include_str!("...")]` value is an unexpanded macro
//! call. Asking for the attribute's string value would report that function as
//! undocumented and then diagnose it, which the contract forbids.

use rustc_ast::{
    ast::{AttrArgs, AttrItem, AttrKind, Attribute},
    token::{Token, TokenKind},
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
/// be diagnosed despite being documented. Matching the shape — an identifier
/// `doc` followed by `=` — counts both forms, which is the direction the
/// contract requires: a false "documented" is silence, while a false
/// "undocumented" is a wrong diagnostic.
fn cfg_attr_args_carry_doc(item: &AttrItem) -> bool {
    let Some(AttrArgs::Delimited(args)) = item.args.unparsed_ref() else {
        return false;
    };
    let mut tokens = args.tokens.iter().peekable();
    // The first comma-separated entry is the configuration predicate; only the
    // entries after it can add documentation. Commas inside a parenthesized
    // predicate, as in `all(a, b)`, are nested and so are not seen here.
    let mut predicate_ended = false;
    while let Some(tree) = tokens.next() {
        match tree {
            TokenTree::Token(
                Token {
                    kind: TokenKind::Comma,
                    ..
                },
                _,
            ) => predicate_ended = true,
            TokenTree::Token(
                Token {
                    kind: TokenKind::Ident(name, _),
                    ..
                },
                _,
            ) if predicate_ended && *name == sym::doc => {
                if matches!(
                    tokens.peek(),
                    Some(TokenTree::Token(
                        Token {
                            kind: TokenKind::Eq,
                            ..
                        },
                        _
                    ))
                ) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}
