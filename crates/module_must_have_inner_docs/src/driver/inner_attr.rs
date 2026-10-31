//! Helpers for inspecting inner attribute contents.
//!
//! This module keeps parsing logic for `#![...]` bodies that is not directly
//! tied to the main lint flow, such as detecting case-mismatched `doc`
//! identifiers and filtering `cfg_attr` wrappers that never supply docs.

use crate::{AttributeBody, ParseInput};

use super::parser;

/// Check whether one meta-list segment names `doc` with incorrect casing.
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

/// Search a `cfg_attr` meta-list for a case-mismatched documentation key.
fn cfg_attr_has_case_incorrect_doc(rest: ParseInput<'_>) -> bool {
    parser::cfg_attr_has_meta_item(rest, segment_has_case_incorrect_doc)
}

/// Extract the contents of a well-formed inner attribute prefix.
fn inner_attribute_body(rest: ParseInput<'_>) -> Option<AttributeBody<'_>> {
    let after_bang = rest.strip_prefix("#!")?;

    let (_, tail) = parser::skip_leading_whitespace(ParseInput::from(after_bang));
    let body = tail.strip_prefix('[')?;

    // Missing `]` is tolerated here: downstream identifier parsing will
    // gracefully reject malformed content by failing to match expected patterns.
    let attr_end = body.find(']').unwrap_or(body.len());
    body.get(..attr_end).map(AttributeBody::from)
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

#[cfg(test)]
mod tests {
    //! Tests for inner attribute parsing.

    use super::is_case_incorrect_doc_inner_attr;
    use crate::ParseInput;
    use rstest::rstest;

    /// Detect case-mismatched documentation attributes inside `cfg_attr`.
    #[rstest]
    #[case(
        "#![cfg_attr(all(feature = \"x\", unix), Doc = \"example\", allow(dead_code))]",
        true
    )]
    #[case(
        "#![cfg_attr(all(feature = \"x\", unix), doc = \"example\", allow(dead_code))]",
        false
    )]
    #[case(
        "#![cfg_attr(feature = \"x\", cfg_attr(unix, DOC = \"example\"))]",
        true
    )]
    #[case(
        r#"#![cfg_attr(feature = "x", deprecated(note = ")"), Doc = "example")]"#,
        true
    )]
    #[case("#![cfg_attr(feature = \"x\", allow(dead_code))]", false)]
    fn detects_case_mismatched_doc_attributes_inside_cfg_attr(
        #[case] attribute: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_case_incorrect_doc_inner_attr(ParseInput::from(attribute)),
            expected,
            "unexpected result for {attribute}"
        );
    }
}
