//! Run-time value-preservation gate for generated rewrites.
//!
//! The rewrite constructor reasons about byte offsets. That reasoning is easy
//! to review and hard to trust: an off-by-one in the offset arithmetic would
//! silently corrupt a user's source. The gate therefore decodes the original
//! literal body and each generated fragment independently, using the compiler's
//! own escape unescaper, and compares the resulting character sequences.
//!
//! `rustc_lexer` implements a source-line join (`\` followed by a line feed)
//! by skipping the escape and the following ASCII whitespace without invoking
//! the callback. Decoding therefore reproduces exactly what the compiler will
//! evaluate, which is what makes this an independent oracle rather than a
//! restatement of the rewriter's own assumptions.
//!
//! # Examples
//!
//! ```
//! use whitaker_string_literals::rewrite::gate::{decoded_value, preserves_value};
//!
//! assert_eq!(decoded_value("a\\\n  b").as_deref(), Some("ab"));
//! assert!(preserves_value("a\\\n  b", &["a", "b"]));
//! assert!(!preserves_value("a\\\n  b", &["a"]));
//! ```

use rustc_lexer::unescape::unescape_str;

/// Decodes the escape sequences of a cooked string literal body.
///
/// Returns `None` when any escape is invalid, which is how a fragment that
/// starts or ends inside another escape is rejected rather than approximated.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::rewrite::gate::decoded_value;
///
/// assert_eq!(decoded_value(r"\u{1F600}").as_deref(), Some("😀"));
/// assert_eq!(decoded_value("\\q"), None);
/// ```
#[must_use]
pub fn decoded_value(body: &str) -> Option<String> {
    let mut value = String::with_capacity(body.len());
    let mut is_valid = true;
    unescape_str(body, &mut |_, result| match result {
        Ok(character) => value.push(character),
        Err(_) => is_valid = false,
    });
    is_valid.then_some(value)
}

/// Returns whether the fragments evaluate to the original body's value.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::rewrite::gate::preserves_value;
///
/// assert!(preserves_value(r"a\tb", &[r"a\t", "b"]));
/// assert!(!preserves_value(r"a\tb", &[r"a\ t", "b"]));
/// ```
#[must_use]
pub fn preserves_value(original: &str, fragments: &[&str]) -> bool {
    let Some(original_value) = decoded_value(original) else {
        return false;
    };
    let mut rewritten = String::with_capacity(original_value.len());
    for fragment in fragments {
        let Some(value) = decoded_value(fragment) else {
            return false;
        };
        rewritten.push_str(&value);
    }
    rewritten == original_value
}

#[cfg(test)]
mod tests {
    //! Tests for the value gate: line joins, ordinary escapes, and the
    //! corruptions the gate exists to catch.

    use rstest::rstest;

    use super::{decoded_value, preserves_value};

    #[test]
    fn decodes_a_line_join_by_removing_it() {
        assert_eq!(
            decoded_value("alpha \\\n    beta").as_deref(),
            Some("alpha beta")
        );
    }

    #[test]
    fn decodes_ordinary_escapes() {
        assert_eq!(decoded_value(r"tab\there").as_deref(), Some("tab\there"));
    }

    #[test]
    fn rejects_a_lone_backslash() {
        assert_eq!(decoded_value(r"trailing\"), None);
    }

    #[rstest]
    #[case::two_fragments("red\\\n blue", &["red", "blue"], true)]
    #[case::empty_fragment("ab", &["a", "", "b"], true)]
    #[case::dropped_character("bravo", &["brav"], false)]
    #[case::duplicated_character("bravo", &["bbravo"], false)]
    #[case::split_escape("\\u{41}", &["\\u{4", "1}"], false)]
    fn preserves_value_compares_evaluated_bytes(
        #[case] original: &str,
        #[case] fragments: &[&str],
        #[case] expected: bool,
    ) {
        assert_eq!(preserves_value(original, fragments), expected);
    }

    #[test]
    fn a_fragment_with_an_invalid_escape_fails_the_gate() {
        assert!(!preserves_value("ab", &["a", "b\\"]));
    }
}
