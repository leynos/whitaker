//! Unit tests for the module-documentation parser.

use super::{cfg_attr_has_meta_item, is_doc_comment, skip_leading_whitespace, take_ident};
use crate::ParseInput;
use proptest::{prelude::*, test_runner::TestRunner};
use rstest::rstest;

/// Verify that the byte offset and remainder account for Unicode whitespace.
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

/// Verify that string contents cannot terminate or split a meta list.
#[rstest]
#[case(
    r#"#![cfg_attr(feature = "x", deprecated(note = ")"), doc = "module") ]"#,
    true
)]
#[case(
    r#"#![cfg_attr(feature = "x", deprecated(note = "escaped \" ), doc = \"fake\""), doc = "module") ]"#,
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

/// Verify that comments containing delimiters do not split a meta list.
#[rstest]
#[case(r#"#![cfg_attr(/* " */ all(), doc = "module") ]"#, true)]
#[case(
    r#"#![cfg_attr(/* outer " , ) /* nested " */ still ) , */ all(), doc = "module") ]"#,
    true
)]
#[case(
    r#"#![cfg_attr(// ")
 all(), doc = "module") ]"#,
    true
)]
fn doc_detection_ignores_delimiters_inside_comments(
    #[case] attribute: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        is_doc_comment(ParseInput::from(attribute)),
        expected,
        "unexpected result for {attribute}"
    );
}

/// Verify that generated nested meta lists expose only real top-level targets.
#[test]
fn cfg_attr_scanners_match_top_level_targets_across_lexical_states() {
    let strategy = (0usize..8, 0usize..5, any::<bool>(), any::<bool>());
    let mut runner = TestRunner::default();

    runner
        .run(
            &strategy,
            |(nested_depth, raw_hash_count, has_target, wrong_case)| {
                let quoted_literal = r#""(, ) doc = \"fake\"""#;
                let hashes = "#".repeat(raw_hash_count);
                let near_closing_quote = if raw_hash_count == 0 {
                    String::new()
                } else {
                    format!("\"{} ", "#".repeat(raw_hash_count - 1))
                };
                let raw_literal =
                    format!("r{hashes}\"(, ) {near_closing_quote}doc = fake\"{hashes}");
                let condition_item = format!("feature = {quoted_literal}, feature = {raw_literal}");
                let nesting = nested_depth + 1;
                let condition = format!(
                    "{}all({condition_item}){}",
                    "all(".repeat(nesting),
                    ")".repeat(nesting)
                );
                let nested_meta = format!("nested(value = {quoted_literal}, raw = {raw_literal})");
                let target = if has_target {
                    let name = if wrong_case { "Doc" } else { "doc" };
                    format!(", {name} = \"module\"")
                } else {
                    String::new()
                };
                let tail = format!("({condition}, {nested_meta}{target})");
                let input = format!("#![cfg_attr{tail}]");
                let expected_doc = has_target && !wrong_case;
                let expected_wrong_case = has_target && wrong_case;

                prop_assert_eq!(
                    is_doc_comment(ParseInput::from(input.as_str())),
                    expected_doc
                );
                prop_assert_eq!(
                    cfg_attr_has_meta_item(ParseInput::from(tail.as_str()), |segment| {
                        take_ident(ParseInput::from(segment))
                            .is_some_and(|(ident, _)| *ident == "Doc")
                    }),
                    expected_wrong_case
                );
                Ok(())
            },
        )
        .expect("generated nested meta lists must preserve top-level detection");
}
