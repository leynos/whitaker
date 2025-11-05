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
