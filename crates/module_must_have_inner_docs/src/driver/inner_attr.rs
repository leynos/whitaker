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

struct CaseDocState {
    depth: usize,
    start: usize,
}

impl CaseDocState {
    fn new() -> Self {
        Self { depth: 0, start: 0 }
    }
}

fn has_case_incorrect_doc_in_meta_list(list: &str) -> bool {
    let mut state = CaseDocState::new();

    for (idx, ch) in list.char_indices() {
        if process_char_for_case_incorrect_doc(list, ch, &mut state, idx) {
            return true;
        }
    }

    segment_has_case_incorrect_doc(&list[state.start..])
}

fn process_char_for_case_incorrect_doc(
    list: &str,
    ch: char,
    state: &mut CaseDocState,
    idx: usize,
) -> bool {
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
            if segment_has_case_incorrect_doc(&list[state.start..idx]) {
                return true;
            }
            state.start = idx + 1;
            false
        }
        _ => false,
    }
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

/// Detects inner attributes like `#![DOC = ...]` or `#![cfg_attr(..., Doc = ...)]`
/// where casing of `doc` is wrong.
pub(super) fn is_case_incorrect_doc_inner_attr(rest: ParseInput<'_>) -> bool {
    let Some(after_bang) = rest.strip_prefix("#!") else {
        return false;
    };
    let (_, tail) = parser::skip_leading_whitespace(ParseInput::from(after_bang));
    let Some(body) = tail.strip_prefix('[') else {
        return false;
    };

    segment_has_case_incorrect_doc(body)
}

fn inner_attribute_body(rest: ParseInput<'_>) -> Option<AttributeBody<'_>> {
    let after_bang = rest.strip_prefix("#!")?;

    let (_, tail) = parser::skip_leading_whitespace(ParseInput::from(after_bang));
    let body = tail.strip_prefix('[')?;

    let attr_end = body.find(']').unwrap_or(body.len());
    Some(AttributeBody::from(&body[..attr_end]))
}

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
