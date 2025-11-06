//! Shared localisation helpers for behaviour and quality tests.
//! Provides argument builders and string post-processing utilities
//! that keep suites aligned and readable.

use common::i18n::{Arguments, FluentValue};
use std::borrow::Cow;

const UNICODE_ISOLATION_MARKS: [char; 2] = ['\u{2068}', '\u{2069}'];

/// Remove the Unicode isolation marks Fluent inserts around formatted arguments.
///
/// # Examples
/// ```ignore
/// let cleaned = strip_isolation_marks("\u{2068}42\u{2069}");
/// assert_eq!(cleaned, "42");
/// ```
pub fn strip_isolation_marks<'a>(text: &'a str) -> Cow<'a, str> {
    if text
        .chars()
        .any(|character| UNICODE_ISOLATION_MARKS.contains(&character))
    {
        Cow::Owned(
            text.chars()
                .filter(|character| !UNICODE_ISOLATION_MARKS.contains(character))
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    }
}

/// Build the canonical localisation arguments used across localisation tests.
///
/// # Examples
/// ```ignore
/// let arguments = default_arguments();
/// assert_eq!(arguments["subject"].as_string(), Some("functions".into()));
/// ```
pub fn default_arguments() -> Arguments<'static> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(Cow::Borrowed("subject"), FluentValue::from("functions"));
    args.insert(
        Cow::Borrowed("attribute"),
        FluentValue::from("#[warn(example)]"),
    );
    args.insert(Cow::Borrowed("lint"), FluentValue::from("module_max_lines"));
    args.insert(
        Cow::Borrowed("module"),
        FluentValue::from("module_max_lines"),
    );
    args.insert(Cow::Borrowed("lines"), FluentValue::from(42_i64));
    args.insert(Cow::Borrowed("limit"), FluentValue::from(12_i64));
    args.insert(Cow::Borrowed("branches"), FluentValue::from(3_i64));
    args.insert(
        Cow::Borrowed("branch_phrase"),
        FluentValue::from("3 branches"),
    );
    args
}

/// Report whether a Fluent source line should be skipped when scanning for new
/// message declarations. Lines that begin with whitespace always belong to the
/// previous declaration (continuations or attributes) so they are ignored.
///
/// # Examples
/// ```ignore
/// assert!(should_skip_line(" attribute = value"));
/// assert!(!should_skip_line("identifier = value"));
/// ```
pub fn should_skip_line(line: &str) -> bool {
    matches!(line.as_bytes().first(), Some(b' ' | b'\t'))
}

/// Extract a valid Fluent identifier from a source line.
///
/// Returns `None` for comments, blank lines, whitespace-prefixed attribute
/// lines, and malformed declarations that lack a non-empty identifier before
/// the first `=` sign.
///
/// # Examples
/// ```ignore
/// assert_eq!(extract_identifier("message = Value"), Some("message".into()));
/// assert_eq!(extract_identifier("  continuation"), None);
/// assert_eq!(extract_identifier("# comment"), None);
/// ```
pub fn extract_identifier(line: &str) -> Option<String> {
    if should_skip_line(line) {
        return None;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.is_empty() {
        return None;
    }
    let (identifier, _) = trimmed.split_once('=')?;
    let id = identifier.trim();
    if id.is_empty() {
        return None;
    }
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_skip_line_detects_leading_whitespace() {
        assert!(should_skip_line(" value"));
        assert!(should_skip_line("\tvalue"));
        assert!(!should_skip_line("value"));
        assert!(!should_skip_line(""));
    }

    #[test]
    fn extract_identifier_handles_basic_messages() {
        assert_eq!(
            extract_identifier("message-id = Value"),
            Some("message-id".to_string())
        );
    }

    #[test]
    fn extract_identifier_rejects_whitespace_and_comments() {
        assert!(extract_identifier(" message = Value").is_none());
        assert!(extract_identifier("# comment").is_none());
        assert!(extract_identifier("").is_none());
    }

    #[test]
    fn extract_identifier_handles_multiple_equals() {
        assert_eq!(
            extract_identifier("message = part = extra"),
            Some("message".to_string())
        );
    }

    #[test]
    fn extract_identifier_rejects_missing_names() {
        assert!(extract_identifier("= value").is_none());
        assert!(extract_identifier("value without equals").is_none());
    }
}
