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

use crate::{AttributeBody, MetaList, ParseInput};

/// Skips leading Unicode whitespace in the input.
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
            return body
                .get(..attr_end)
                .is_some_and(|attr_body| is_doc_attr(AttributeBody::from(attr_body)));
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

// Detects documentation by matching a `doc` ident directly or inside `cfg_attr`.
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

const fn is_ident_start(ch: char) -> bool {
    // Ident parsing is intentionally ASCII-only; we only need to recognize
    // built-in attribute names such as `doc` and `cfg_attr`.
    ch == '_' || ch.is_ascii_alphabetic()
}

const fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

pub(super) fn cfg_attr_has_doc(tail: ParseInput<'_>) -> bool {
    cfg_attr_has_meta_item(tail, segment_is_doc)
}

/// Checks the attributes after the condition in a `cfg_attr` meta list.
pub(super) fn cfg_attr_has_meta_item(
    tail: ParseInput<'_>,
    mut matches_attribute: impl FnMut(&str) -> bool,
) -> bool {
    let Some(meta_list) = cfg_attr_meta_list(tail) else {
        return false;
    };

    meta_list_contains(meta_list, &mut matches_attribute)
}

fn cfg_attr_meta_list(tail: ParseInput<'_>) -> Option<MetaList<'_>> {
    let (_, trimmed) = skip_leading_whitespace(tail);
    let content = trimmed.strip_prefix('(')?;
    let (condition_comma, close_idx) = cfg_attr_delimiters(content)?;
    let attributes_start = condition_comma.checked_add(1)?;
    let attributes = content.get(attributes_start..close_idx)?;

    Some(MetaList::from(attributes))
}

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
    literal_state: LiteralState,
}

impl CfgAttrScanState {
    fn new() -> Self {
        Self {
            depth: 1,
            condition_comma: None,
            literal_state: LiteralState::default(),
        }
    }

    fn consume(&mut self, content: &str, idx: usize, ch: char) -> bool {
        if self.literal_state.consumes(content, idx, ch) {
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

#[derive(Default)]
enum LiteralState {
    #[default]
    Outside,
    Quoted {
        is_escaped: bool,
    },
    Raw(RawStringState),
}

struct RawStringState {
    opening_quote: usize,
    hash_count: usize,
}

impl LiteralState {
    fn consumes(&mut self, input: &str, idx: usize, ch: char) -> bool {
        match std::mem::take(self) {
            Self::Outside => match literal_state_at(input, idx, ch) {
                Self::Outside => false,
                next_state => {
                    *self = next_state;
                    true
                }
            },
            Self::Quoted { is_escaped } => {
                *self = next_quoted_state(is_escaped, ch);
                true
            }
            Self::Raw(raw_string) => {
                *self = raw_string.next(input, idx, ch);
                true
            }
        }
    }
}

fn literal_state_at(input: &str, idx: usize, ch: char) -> LiteralState {
    match (raw_string_start(input, idx), ch) {
        (Some((opening_quote, hash_count)), _) => LiteralState::Raw(RawStringState {
            opening_quote,
            hash_count,
        }),
        (None, '"') => LiteralState::Quoted { is_escaped: false },
        (None, _) => LiteralState::Outside,
    }
}

fn next_quoted_state(is_escaped: bool, ch: char) -> LiteralState {
    match (is_escaped, ch) {
        (true, _) => LiteralState::Quoted { is_escaped: false },
        (false, '\\') => LiteralState::Quoted { is_escaped: true },
        (false, '"') => LiteralState::Outside,
        (false, _) => LiteralState::Quoted { is_escaped: false },
    }
}

impl RawStringState {
    fn next(self, input: &str, idx: usize, ch: char) -> LiteralState {
        match (
            idx == self.opening_quote,
            ch == '"',
            raw_string_ends(input, idx, self.hash_count),
        ) {
            (false, true, true) => LiteralState::Outside,
            _ => LiteralState::Raw(self),
        }
    }
}

fn raw_string_start(input: &str, idx: usize) -> Option<(usize, usize)> {
    let suffix = input.get(idx..)?;
    let prefix_length = suffix
        .strip_prefix("br")
        .map(|_| 2)
        .or_else(|| suffix.strip_prefix('r').map(|_| 1))?;
    let raw_content = suffix.get(prefix_length..)?;
    let hash_count = raw_content.bytes().take_while(|byte| *byte == b'#').count();
    let opening_quote = idx.saturating_add(prefix_length).saturating_add(hash_count);

    raw_content
        .get(hash_count..)
        .filter(|tail| tail.starts_with('"'))
        .map(|_| (opening_quote, hash_count))
}

fn raw_string_ends(input: &str, idx: usize, hash_count: usize) -> bool {
    input
        .get(idx.saturating_add(1)..)
        .and_then(|tail| tail.as_bytes().get(..hash_count))
        .is_some_and(|hashes| hashes.iter().all(|byte| *byte == b'#'))
}

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
    literal_state: LiteralState,
    matches_attribute: MatchesAttribute,
}

impl<'list, MatchesAttribute> MetaListScanner<'list, MatchesAttribute>
where
    MatchesAttribute: FnMut(&str) -> bool,
{
    fn new(list: &'list str, matches_attribute: MatchesAttribute) -> Self {
        Self {
            list,
            depth: 0,
            start: 0,
            literal_state: LiteralState::default(),
            matches_attribute,
        }
    }

    fn matches_at_separator(&mut self, idx: usize, ch: char) -> bool {
        if self.literal_state.consumes(self.list, idx, ch) {
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

    fn matches_remaining(&mut self) -> bool {
        meta_item_matches(
            self.list,
            self.start,
            self.list.len(),
            &mut self.matches_attribute,
        )
    }
}

fn is_top_level_meta_separator(ch: char, depth: &mut usize) -> bool {
    match ch {
        '(' => *depth += 1,
        ')' => *depth = depth.saturating_sub(1),
        ',' => return *depth == 0,
        _ => {}
    }
    false
}

fn meta_item_matches(
    list: &str,
    start: usize,
    end: usize,
    mut matches_attribute: impl FnMut(&str) -> bool,
) -> bool {
    list.get(start..end).is_some_and(&mut matches_attribute)
}

fn segment_is_doc(segment: &str) -> bool {
    is_doc_ident(ParseInput::from(segment))
}

#[cfg(test)]
mod tests {
    //! Unit tests for parsing helpers.

    use super::{is_doc_comment, skip_leading_whitespace};
    use crate::ParseInput;
    use rstest::rstest;

    #[rstest]
    #[case("", 0, "")]
    #[case(" \t\n", " \t\n".len(), "")]
    #[case("//! docs", 0, "//! docs")]
    #[case("\u{2003}x", "\u{2003}".len(), "x")]
    #[case("\u{00A0}\u{2003}//! docs", "\u{00A0}\u{2003}".len(), "//! docs")]
    #[case("\u{00A0}\u{2003}\t\n", "\u{00A0}\u{2003}\t\n".len(), "")]
    fn skip_leading_whitespace_handles_unicode(
        #[case] input: &str,
        #[case] expected_offset: usize,
        #[case] expected_rest: &str,
    ) {
        let (offset, rest) = skip_leading_whitespace(ParseInput::from(input));

        assert_eq!(offset, expected_offset);
        assert_eq!(rest.as_str(), expected_rest);
    }

    #[rstest]
    #[case(
        r#"#![cfg_attr(feature = "x", deprecated(note = ")"), doc = "module")]"#,
        true
    )]
    #[case(
        r#"#![cfg_attr(feature = "x", deprecated(note = "escaped \" ), doc = \"fake\""), doc = "module")]"#,
        true
    )]
    #[case(
        r##"#![cfg_attr(feature = "x", deprecated(note = r#"), doc = "fake"#), doc = "module") ]"##,
        true
    )]
    #[case(
        r#"#![cfg_attr(feature = "x", deprecated(note = "no, doc = \"fake\""))]"#,
        false
    )]
    fn doc_detection_ignores_delimiters_inside_string_literals(
        #[case] attribute: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_doc_comment(ParseInput::from(attribute)),
            expected,
            "unexpected result for {attribute}"
        );
    }
}
