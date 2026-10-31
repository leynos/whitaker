//! Parsing helpers for module doc detection.
//!
//! This module tokenizes raw attribute text to spot inner doc comments, walks
//! `cfg_attr` wrappers to inspect their contained meta items, and skips leading
//! whitespace before parsing. It looks for doc tokens (`//!` line comments and
//! `#![doc = \"...\"]` style attributes) while ignoring commas inside nested
//! parentheses and string literals when dissecting meta lists. One crate-private
//! meta-list walker serves the documentation and case-mismatch checks, while
//! each caller owns its identifier policy. Key helpers:
//! - `skip_leading_whitespace`: advances a text cursor past Unicode whitespace.
//! - `is_doc_comment`: recognizes leading doc comments or doc attributes,
//!   including those wrapped in `cfg_attr`.
//!
//! These utilities underpin the lint that determines whether a module has the
//! required leading inner docs.

use super::literal::LexicalState;
use crate::{AttributeBody, MetaList, ParseInput};

/// Skip leading Unicode whitespace in the input.
///
/// Returns the byte offset of the first non-whitespace character and the
/// remaining input after whitespace. If the input is entirely whitespace or
/// empty, the offset equals the input length and the remaining input is empty.
///
/// This uses `trim_start_matches` to compute the leading whitespace span while
/// preserving the original byte offsets.
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
///
/// Unicode whitespace is also handled:
///
/// ```
/// # use module_must_have_inner_docs::ParseInput;
/// # use module_must_have_inner_docs::parser::skip_leading_whitespace;
/// let input = ParseInput::from("\u{00A0}\u{2003}hello");
/// let (offset, rest) = skip_leading_whitespace(input);
/// assert_eq!(offset, 5); // 2 bytes + 3 bytes.
/// assert_eq!(rest.as_str(), "hello");
/// ```
pub(super) fn skip_leading_whitespace(snippet: ParseInput<'_>) -> (usize, ParseInput<'_>) {
    let snippet_str = snippet.as_str();
    let trimmed = snippet_str.trim_start_matches(char::is_whitespace);
    let byte_offset = snippet_str.len().saturating_sub(trimmed.len());

    (byte_offset, ParseInput::from(trimmed))
}

/// Determine whether the input starts with a module-level doc comment.
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
            return body
                .get(..attr_end)
                .is_some_and(|attr_body| is_doc_attr(AttributeBody::from(attr_body)));
        }
    }
    false
}

/// Recognize direct `doc` attributes and wrappers that contain a `doc` entry.
fn is_doc_attr(attr_body: AttributeBody<'_>) -> bool {
    is_doc_ident(ParseInput::from(*attr_body))
}

/// Extract the leading identifier from the input, skipping any leading
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
pub(super) fn take_ident(input: ParseInput<'_>) -> Option<(ParseInput<'_>, ParseInput<'_>)> {
    let (_, trimmed) = skip_leading_whitespace(input);
    let trimmed_str = trimmed.as_str();
    let mut iter = trimmed_str.char_indices();
    let (start, first_ch) = iter.next()?;
    if !is_ident_start(first_ch) {
        return None;
    }

    let mut end = start + first_ch.len_utf8();
    for (idx, ch) in iter {
        if is_ident_continue(ch) {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    let (ident, remainder) = trimmed_str.split_at_checked(end)?;
    Some((ParseInput::from(ident), ParseInput::from(remainder)))
}

/// Find a direct `doc` identifier or a `cfg_attr` wrapper containing one.
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

/// Recognize the first character allowed in an attribute identifier.
const fn is_ident_start(ch: char) -> bool {
    // Ident parsing is intentionally ASCII-only; we only need to recognize
    // built-in attribute names such as `doc` and `cfg_attr`.
    ch == '_' || ch.is_ascii_alphabetic()
}

/// Recognize subsequent characters allowed in an attribute identifier.
const fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Detect a `doc` item in a `cfg_attr` meta list.
pub(super) fn cfg_attr_has_doc(tail: ParseInput<'_>) -> bool {
    cfg_attr_has_meta_item(tail, segment_is_doc)
}

/// Check `cfg_attr` items with a caller-supplied name predicate.
pub(super) fn cfg_attr_has_meta_item(
    tail: ParseInput<'_>,
    mut matches_attribute: impl FnMut(&str) -> bool,
) -> bool {
    let Some(meta_list) = cfg_attr_meta_list(tail) else {
        return false;
    };

    meta_list_contains(meta_list, &mut matches_attribute)
}

/// Extract the attribute portion after the condition in a `cfg_attr` list.
///
/// Keep the condition separate so callers only inspect attributes activated by
/// the wrapper.
fn cfg_attr_meta_list(tail: ParseInput<'_>) -> Option<MetaList<'_>> {
    let (_, trimmed) = skip_leading_whitespace(tail);
    let content = trimmed.strip_prefix('(')?;
    let (condition_comma, close_idx) = cfg_attr_delimiters(content)?;
    let attributes_start = condition_comma.checked_add(1)?;
    let attributes = content.get(attributes_start..close_idx)?;

    Some(MetaList::from(attributes))
}

/// Find the condition comma and outer closing parenthesis in `cfg_attr` text.
///
/// Ignore delimiters inside nested meta items, strings, and comments.
fn cfg_attr_delimiters(content: &str) -> Option<(usize, usize)> {
    let mut scan_state = CfgAttrScanState::new();

    for (idx, ch) in content.char_indices() {
        if scan_state.consume(content, idx, ch) {
            return scan_state.condition_comma.map(|comma| (comma, idx));
        }
    }

    None
}

struct CfgAttrScanState {
    depth: usize,
    condition_comma: Option<usize>,
    lexical_state: LexicalState,
}

impl CfgAttrScanState {
    /// Create a scanner positioned inside the outer `cfg_attr` parentheses.
    fn new() -> Self {
        Self {
            depth: 1,
            condition_comma: None,
            lexical_state: LexicalState::default(),
        }
    }

    /// Track nested parentheses and return true at the outer closing one.
    fn consume(&mut self, content: &str, idx: usize, ch: char) -> bool {
        if self.lexical_state.consumes(content, idx, ch) {
            return false;
        }

        match (ch, self.depth, self.condition_comma.is_none()) {
            ('(', _, _) => self.depth += 1,
            (')', 1, _) => return true,
            (')', _, _) => self.depth = self.depth.saturating_sub(1),
            (',', 1, true) => self.condition_comma = Some(idx),
            _ => {}
        }
        false
    }
}

/// Check the attributes in a meta list with a caller-supplied predicate.
pub(super) fn meta_list_contains(
    list: MetaList<'_>,
    matches_attribute: impl FnMut(&str) -> bool,
) -> bool {
    let mut scanner = MetaListScanner::new(*list, matches_attribute);

    for (idx, ch) in scanner.list.char_indices() {
        if scanner.matches_at_separator(idx, ch) {
            return true;
        }
    }

    scanner.matches_remaining()
}

struct MetaListScanner<'list, MatchesAttribute> {
    list: &'list str,
    depth: usize,
    start: usize,
    lexical_state: LexicalState,
    matches_attribute: MatchesAttribute,
}

impl<'list, MatchesAttribute> MetaListScanner<'list, MatchesAttribute>
where
    MatchesAttribute: FnMut(&str) -> bool,
{
    /// Create a scanner for a comma-separated meta list.
    fn new(list: &'list str, matches_attribute: MatchesAttribute) -> Self {
        Self {
            list,
            depth: 0,
            start: 0,
            lexical_state: LexicalState::default(),
            matches_attribute,
        }
    }

    /// Check a top-level separator and test the preceding meta item.
    fn matches_at_separator(&mut self, idx: usize, ch: char) -> bool {
        if self.lexical_state.consumes(self.list, idx, ch) {
            return false;
        }

        if is_top_level_meta_separator(ch, &mut self.depth) {
            let matches =
                meta_item_matches(self.list, self.start, idx, &mut self.matches_attribute);
            self.start = idx.saturating_add(1);
            return matches;
        }
        false
    }

    /// Test the final meta item after the last top-level separator.
    fn matches_remaining(&mut self) -> bool {
        meta_item_matches(
            self.list,
            self.start,
            self.list.len(),
            &mut self.matches_attribute,
        )
    }
}

/// Update parenthesis depth and identify top-level commas.
fn is_top_level_meta_separator(ch: char, depth: &mut usize) -> bool {
    match ch {
        '(' => *depth += 1,
        ')' => *depth = depth.saturating_sub(1),
        ',' => return *depth == 0,
        _ => {}
    }
    false
}

/// Apply the attribute predicate to one checked meta-list slice.
fn meta_item_matches(
    list: &str,
    start: usize,
    end: usize,
    mut matches_attribute: impl FnMut(&str) -> bool,
) -> bool {
    list.get(start..end).is_some_and(&mut matches_attribute)
}

/// Match a meta-list segment against the module `doc` identifier policy.
fn segment_is_doc(segment: &str) -> bool {
    is_doc_ident(ParseInput::from(segment))
}

#[cfg(test)]
mod tests;
