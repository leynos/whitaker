//! Parsing helpers for module doc detection.
//!
//! This module tokenizes raw attribute text to spot inner doc comments, walks
//! `cfg_attr` wrappers to inspect their contained meta items, and skips leading
//! whitespace before parsing. It looks for doc tokens (`//!` line comments and
//! `#![doc = \"...\"]` style attributes) while ignoring commas inside nested
//! parentheses when dissecting meta lists. Key helpers:
//! - `skip_leading_whitespace`: advances a text cursor past ASCII whitespace.
//! - `is_doc_comment`: recognises leading doc comments or doc attributes,
//!   including those wrapped in `cfg_attr`.
//!
//! These utilities underpin the lint that determines whether a module has the
//! required leading inner docs.

use crate::{AttributeBody, MetaList, ParseInput};

/// Skips leading ASCII whitespace in the input.
///
/// Returns the byte offset of the first non-whitespace character and the
/// remaining input after whitespace. If the input is entirely whitespace or
/// empty, the offset equals the input length and the remaining input is empty.
///
/// # Examples
///
/// ```
/// # use module_must_have_inner_docs::ParseInput;
/// # use module_must_have_inner_docs::parser::skip_leading_whitespace;
/// let input = ParseInput::from("  hello");
/// let (offset, rest) = skip_leading_whitespace(input);
/// assert_eq!(offset, 2);
/// assert_eq!(rest.as_str(), "hello");
/// ```
pub(super) fn skip_leading_whitespace<'a>(snippet: ParseInput<'a>) -> (usize, ParseInput<'a>) {
    let snippet_str = snippet.as_str();
    let bytes = snippet_str.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    (offset, ParseInput::from(&snippet_str[offset..]))
}

/// Determines whether the input starts with a module-level doc comment.
///
/// Recognizes two forms:
/// - Line doc comments: `//! ...`
/// - Inner attribute docs: `#![doc = "..."]` or `#![ doc = "..." ]`
///
/// The attribute form tolerates whitespace between `#!` and `[`, and supports
/// `cfg_attr` wrapping.
///
/// # Examples
///
/// ```
/// # use module_must_have_inner_docs::ParseInput;
/// # use module_must_have_inner_docs::parser::is_doc_comment;
/// assert!(is_doc_comment(ParseInput::from("//! Module doc")));
/// assert!(is_doc_comment(ParseInput::from("#![doc = \"Module\"]")));
/// assert!(!is_doc_comment(ParseInput::from("// Regular comment")));
/// ```
pub(super) fn is_doc_comment(rest: ParseInput<'_>) -> bool {
    if rest.starts_with("//!") {
        return true;
    }
    if let Some(after_bang) = rest.strip_prefix("#!") {
        let (_, tail) = skip_leading_whitespace(ParseInput::from(after_bang));
        if let Some(body) = tail.strip_prefix('[') {
            let attr_end = body.find(']').unwrap_or(body.len());
            let attr_body = AttributeBody::from(&body[..attr_end]);
            return is_doc_attr(attr_body);
        }
    }
    false
}

// Returns true for direct `doc` attributes and for `cfg_attr` wrappers that
// contain a `doc` entry.
fn is_doc_attr(attr_body: AttributeBody<'_>) -> bool {
    is_doc_ident(ParseInput::from(*attr_body))
}

/// Extracts the leading identifier from the input, skipping any leading
/// whitespace.
///
/// An identifier starts with `_` or an ASCII letter and continues with `_` or
/// ASCII alphanumerics. Returns the identifier and the remaining input, or
/// `None` when no identifier is present.
///
/// # Examples
///
/// ```
/// # use module_must_have_inner_docs::ParseInput;
/// # use module_must_have_inner_docs::parser::take_ident;
/// let input = ParseInput::from("  foo_bar(baz)");
/// let Some((ident, rest)) = take_ident(input) else { panic!() };
/// assert_eq!(*ident, "foo_bar");
/// assert_eq!(rest.as_str(), "(baz)");
///
/// assert!(take_ident(ParseInput::from("  123")).is_none());
/// ```
pub(super) fn take_ident<'a>(input: ParseInput<'a>) -> Option<(ParseInput<'a>, ParseInput<'a>)> {
    let (_, trimmed) = skip_leading_whitespace(input);
    let trimmed_str = trimmed.as_str();
    let mut iter = trimmed_str.char_indices();
    let (start, ch) = iter.next()?;
    if !is_ident_start(ch) {
        return None;
    }

    let mut end = start + ch.len_utf8();
    for (idx, ch) in iter {
        if is_ident_continue(ch) {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    let ident = ParseInput::from(&trimmed_str[..end]);
    Some((ident, ParseInput::from(&trimmed_str[end..])))
}

fn is_doc_ident(input: ParseInput<'_>) -> bool {
    let Some((ident, tail)) = take_ident(input) else {
        return false;
    };

    if *ident == "doc" {
        return true;
    }

    if *ident == "cfg_attr" {
        return cfg_attr_has_doc(tail);
    }

    false
}

fn is_ident_start(ch: char) -> bool {
    // Ident parsing is intentionally ASCII-only; we only need to recognise
    // built-in attribute names such as `doc` and `cfg_attr`.
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

pub(super) fn cfg_attr_has_doc(tail: ParseInput<'_>) -> bool {
    let (_, tail) = skip_leading_whitespace(tail);
    let Some(args) = tail.strip_prefix('(') else {
        return false;
    };
    let Some(close_idx) = args.rfind(')') else {
        return false;
    };
    let meta_list = &args[..close_idx];

    has_doc_in_meta_list_after_first(MetaList::from(meta_list))
}

fn has_doc_in_meta_list_after_first(list: MetaList<'_>) -> bool {
    let list_str = *list;
    let mut depth: usize = 0;
    let mut start = 0;
    let mut seen_comma = false;

    for (idx, ch) in list_str.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if seen_comma && segment_is_doc(&list_str[start..idx]) {
                    return true;
                }
                seen_comma = true;
                start = idx + 1;
            }
            _ => {}
        }
    }

    seen_comma && segment_is_doc(&list_str[start..])
}

fn segment_is_doc(segment: &str) -> bool {
    is_doc_ident(ParseInput::from(segment))
}
