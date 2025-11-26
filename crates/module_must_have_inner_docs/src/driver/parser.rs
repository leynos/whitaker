//! Parsing helpers for module doc detection.
//!
//! This module tokenises raw attribute text to spot inner doc comments, walks
//! `cfg_attr` wrappers to inspect their contained meta items, and skips leading
//! whitespace before parsing. It looks for doc tokens (`//!` line comments and
//! `#![doc = \"...\"]` style attributes) while ignoring commas inside nested
//! parentheses when dissecting meta lists. Key helpers:
//! - `skip_leading_whitespace`: advances a text cursor past ASCII whitespace.
//! - `is_doc_comment`: recognises leading doc comments or doc attributes,
//!   including those wrapped in `cfg_attr`.
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
/// Recognises two forms:
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
    let Some((ident, tail)) = take_ident(ParseInput::from(*attr_body)) else {
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
fn take_ident<'a>(input: ParseInput<'a>) -> Option<(ParseInput<'a>, ParseInput<'a>)> {
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

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn cfg_attr_has_doc(rest: ParseInput<'_>) -> bool {
    let (_, trimmed) = skip_leading_whitespace(rest);
    // Parse `cfg_attr(condition, attr1, attr2, ...)`; after `(` comes the
    // condition, then a comma, then the attribute list.
    let Some(content) = trimmed.strip_prefix('(') else {
        return false;
    };

    let mut depth: usize = 1;
    let mut first_comma = None;
    let mut closing_paren = None;

    // Walk the chars, tracking parentheses so we can ignore commas inside nested
    // cfg expressions. The first comma at depth 1 separates the condition from
    // the attribute list; the closing paren at depth 0 marks the end.
    for (idx, ch) in content.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    closing_paren = Some(idx);
                    break;
                }
            }
            ',' if depth == 1 && first_comma.is_none() => first_comma = Some(idx),
            _ => {}
        }
    }

    let Some(attr_section_start) = first_comma else {
        return false;
    };
    let Some(close_idx) = closing_paren else {
        return false;
    };

    // `args` holds everything inside the outer parens; slice from the comma
    // boundary to the closing paren to obtain the attribute list as a MetaList.
    let args = &content[..close_idx];
    let attr_section = &args[attr_section_start + 1..];
    has_doc_in_meta_list(MetaList::from(attr_section))
}

struct ParserState {
    depth: usize,
    start: usize,
}

impl ParserState {
    fn new() -> Self {
        Self { depth: 0, start: 0 }
    }
}

fn has_doc_in_meta_list(list: MetaList<'_>) -> bool {
    // Scan comma-separated meta items at depth 0, using depth tracking to skip
    // commas inside nested parentheses. `process_char_for_doc` updates state and
    // early-returns when a doc segment is found; the final call checks the tail
    // after the last comma.
    let list_str = *list;
    let mut state = ParserState::new();

    for (idx, ch) in list_str.char_indices() {
        if process_char_for_doc(list_str, ch, &mut state, idx) {
            return true;
        }
    }

    segment_is_doc(&list_str[state.start..])
}

fn process_char_for_doc(list_str: &str, ch: char, state: &mut ParserState, idx: usize) -> bool {
    match ch {
        '(' => {
            state.depth += 1;
            false
        }
        ')' => {
            state.depth = state.depth.saturating_sub(1);
            false
        }
        ',' if state.depth == 0 => {
            if segment_is_doc(&list_str[state.start..idx]) {
                return true;
            }
            state.start = idx + 1;
            false
        }
        _ => false,
    }
}

fn segment_is_doc(segment: &str) -> bool {
    let Some((ident, _)) = take_ident(ParseInput::from(segment)) else {
        return false;
    };

    *ident == "doc"
}
