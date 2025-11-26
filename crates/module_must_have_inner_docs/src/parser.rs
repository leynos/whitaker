//! Parsing helpers for module doc detection.

use crate::{AttributeBody, MetaList, ParseInput};

pub(super) fn skip_leading_whitespace<'a>(snippet: ParseInput<'a>) -> (usize, ParseInput<'a>) {
    let snippet_str = snippet.as_str();
    let bytes = snippet_str.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    (offset, ParseInput::from(&snippet_str[offset..]))
}

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
    has_doc_in_meta_list(MetaList::from(attr_section))
}

fn has_doc_in_meta_list(list: MetaList<'_>) -> bool {
    let list_str = *list;
    let mut depth: usize = 0;
    let mut start = 0;

    for (idx, ch) in list_str.char_indices() {
        if process_char_for_doc(list_str, ch, &mut depth, &mut start, idx) {
            return true;
        }
    }

    segment_is_doc(&list_str[start..])
}

fn process_char_for_doc(
    list_str: &str,
    ch: char,
    depth: &mut usize,
    start: &mut usize,
    idx: usize,
) -> bool {
    match ch {
        '(' => {
            *depth += 1;
            false
        }
        ')' => {
            *depth = depth.saturating_sub(1);
            false
        }
        ',' if *depth == 0 => {
            if segment_is_doc(&list_str[*start..idx]) {
                return true;
            }
            *start = idx + 1;
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
