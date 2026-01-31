//! Helpers for inspecting inner attribute contents.
//!
//! This module keeps parsing logic for `#![...]` bodies that is not directly
//! tied to the main lint flow, such as detecting case-mismatched `doc`
//! identifiers and filtering `cfg_attr` wrappers that never supply docs.

use crate::{AttributeBody, ParseInput};

use super::parser;

fn segment_has_case_incorrect_doc(segment: &str) -> bool {
    let Some((ident, tail)) = parser::take_ident(ParseInput::from(segment)) else {
        return false;
    };

    if ident.eq_ignore_ascii_case("doc") {
        return *ident != "doc";
    }

    if *ident == "cfg_attr" {
        return cfg_attr_has_case_incorrect_doc(tail);
    }

    false
}

struct ParserState {
    depth: usize,
    start: usize,
}

fn has_case_incorrect_doc_in_meta_list(list: &str) -> bool {
    let mut state = ParserState { depth: 0, start: 0 };

    for (idx, ch) in list.char_indices() {
        if handle_character(ch, &mut state, list, idx) {
            return true;
        }
    }

    segment_has_case_incorrect_doc(&list[state.start..])
}

// Extracted to reduce nested complexity in `has_case_incorrect_doc_in_meta_list`.
fn handle_character(ch: char, state: &mut ParserState, list: &str, idx: usize) -> bool {
    match ch {
        '(' => state.depth += 1,
        ')' => state.depth = state.depth.saturating_sub(1),
        ',' if state.depth == 0 => {
            if segment_has_case_incorrect_doc(&list[state.start..idx]) {
                return true;
            }
            state.start = idx + 1;
        }
        _ => {}
    }
    false
}

fn cfg_attr_has_case_incorrect_doc(rest: ParseInput<'_>) -> bool {
    let (_, trimmed) = parser::skip_leading_whitespace(rest);
    let Some(content) = trimmed.strip_prefix('(') else {
        return false;
    };

    let mut depth: usize = 1;
    let mut first_comma = None;
    let mut closing_paren = None;

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

    let args = &content[..close_idx];
    let attr_section = &args[attr_section_start + 1..];
    has_case_incorrect_doc_in_meta_list(attr_section)
}

fn inner_attribute_body(rest: ParseInput<'_>) -> Option<AttributeBody<'_>> {
    let after_bang = rest.strip_prefix("#!")?;

    let (_, tail) = parser::skip_leading_whitespace(ParseInput::from(after_bang));
    let body = tail.strip_prefix('[')?;

    // Missing `]` is tolerated here: downstream identifier parsing will
    // gracefully reject malformed content by failing to match expected patterns.
    let attr_end = body.find(']').unwrap_or(body.len());
    Some(AttributeBody::from(&body[..attr_end]))
}

/// Detects inner attributes like `#![DOC = ...]` or `#![cfg_attr(..., Doc = ...)]`
/// where casing of `doc` is wrong.
///
/// # Examples
///
/// ```ignore
/// use crate::ParseInput;
/// use crate::driver::inner_attr::is_case_incorrect_doc_inner_attr;
///
/// // Incorrect casing returns true
/// let input = ParseInput::from("#![DOC = \"example\"]");
/// assert!(is_case_incorrect_doc_inner_attr(input));
///
/// // Correct casing returns false
/// let input = ParseInput::from("#![doc = \"example\"]");
/// assert!(!is_case_incorrect_doc_inner_attr(input));
/// ```
pub(super) fn is_case_incorrect_doc_inner_attr(rest: ParseInput<'_>) -> bool {
    let Some(body) = inner_attribute_body(rest) else {
        return false;
    };

    segment_has_case_incorrect_doc(*body)
}

/// Checks whether a `cfg_attr` inner attribute omits any `doc` attribute.
///
/// # Examples
///
/// ```ignore
/// use crate::ParseInput;
/// use crate::driver::inner_attr::is_cfg_attr_without_doc;
///
/// // cfg_attr without doc returns true
/// let input = ParseInput::from("#![cfg_attr(feature = \"x\", inline)]");
/// assert!(is_cfg_attr_without_doc(input));
///
/// // cfg_attr with doc returns false
/// let input = ParseInput::from("#![cfg_attr(feature = \"x\", doc = \"example\")]");
/// assert!(!is_cfg_attr_without_doc(input));
/// ```
pub(super) fn is_cfg_attr_without_doc(rest: ParseInput<'_>) -> bool {
    let Some(body) = inner_attribute_body(rest) else {
        return false;
    };

    let Some((ident, tail)) = parser::take_ident(ParseInput::from(*body)) else {
        return false;
    };

    // A doc-less `cfg_attr` wrapper leaves the module undocumented even when
    // the condition holds, so treat it the same as having no inner attributes.
    *ident == "cfg_attr" && !parser::cfg_attr_has_doc(tail)
}
