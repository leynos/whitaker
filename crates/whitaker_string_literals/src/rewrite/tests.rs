//! Unit tests for rewrite construction and applicability checks.

use super::{ConcatRewrite, Newline, is_concat_macro_expression};
use crate::{
    continuation::{Continuation, ContinuationKind},
    facts::{BodyOffset, LiteralBody},
};

fn interior_joins(body: LiteralBody<'_>) -> Vec<Continuation> {
    body.scan()
        .into_iter()
        .filter(|continuation| continuation.kind == ContinuationKind::Join)
        .collect()
}

#[test]
fn splits_at_every_join_and_copies_original_escape_spelling() {
    let body = LiteralBody::new("a\\\"b\\\\c\\\n d\\\n e");
    let joins = interior_joins(body);
    let rewrite = ConcatRewrite::build(body, &joins).expect("the body contains safe joins");

    assert_eq!(rewrite.fragments(), &["a\\\"b\\\\c", "d", "e"]);
    assert_eq!(
        rewrite.join_offsets(),
        &[BodyOffset::from_index(7), BodyOffset::from_index(11)]
    );
}

#[test]
fn rejects_empty_join_lists() {
    let plain = LiteralBody::new("ordinary text");

    assert!(ConcatRewrite::build(plain, &[]).is_none());
}

#[test]
fn rejects_layout_trims() {
    let leading_layout = LiteralBody::new("\\\n text");
    let layout = leading_layout.scan();

    assert!(ConcatRewrite::build(leading_layout, &layout).is_none());
}

#[test]
fn rejects_unlexable_original_string_tokens() {
    let body = LiteralBody::new("alpha\" \\\nbeta");
    let joins = interior_joins(body);

    assert_eq!(joins.len(), 1);
    assert!(ConcatRewrite::build(body, &joins).is_none());
}

#[test]
fn rejects_ranges_that_do_not_match_the_scanner_output() {
    let body = LiteralBody::new("alpha\\\nbeta");
    let forged = Continuation {
        escape_range: BodyOffset::from_index(0)..BodyOffset::from_index(2),
        skipped_whitespace_range: BodyOffset::from_index(2)..BodyOffset::from_index(2),
        kind: ContinuationKind::Join,
    };

    assert!(ConcatRewrite::build(body, &[forged]).is_none());
}

#[test]
fn rejects_a_partial_set_of_detected_joins() {
    let body = LiteralBody::new("first\\\nsecond\\\nthird");
    let joins = interior_joins(body);

    assert_eq!(joins.len(), 2);
    let first_join = joins.first().expect("the body contains two joins");
    let partial_joins = std::slice::from_ref(first_join);
    assert!(ConcatRewrite::build(body, partial_joins).is_none());
}

#[test]
fn renders_a_single_line_replacement_when_it_fits() {
    let body = LiteralBody::new("alpha\\\nbeta");
    let joins = interior_joins(body);
    let rewrite = ConcatRewrite::build(body, &joins).expect("safe join");

    assert_eq!(
        rewrite.render(0, Newline::Lf),
        "concat!(\"alpha\", \"beta\")"
    );
}

#[test]
fn renders_a_multiline_replacement_at_the_original_indentation() {
    let body_source = format!("{}\\\n{}", "a".repeat(55), "b".repeat(55));
    let body = LiteralBody::new(&body_source);
    let joins = interior_joins(body);
    let rewrite = ConcatRewrite::build(body, &joins).expect("safe join");
    let rendered = rewrite.render(4, Newline::CrLf);

    assert!(rendered.starts_with("concat!(\r\n        \""));
    assert!(rendered.contains(",\r\n        \""));
    assert!(rendered.ends_with(",\r\n    )"));
}

#[rstest::rstest]
#[case("concat!(\"a\", \"b\")", true)]
#[case("concat!(\"a\", \"b\",)", true)]
#[case("concat!(\"a\")", false)]
#[case("concat!(\"a\", \"b\");", false)]
#[case("concatx!(\"a\", \"b\")", false)]
fn validates_the_generated_macro_expression(#[case] source: &str, #[case] expected: bool) {
    assert_eq!(is_concat_macro_expression(source), expected);
}
