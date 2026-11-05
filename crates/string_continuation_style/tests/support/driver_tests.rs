//! In-process regression checks for adapter safety boundaries.

use rustc_span::{BytePos, DUMMY_SP};
use whitaker_string_literals::rewrite::{cooked_literal_body, gate::preserves_value};

use super::span::body_range_span;

#[test]
fn whole_token_and_join_subspans_stay_inside_the_literal() {
    let literal = DUMMY_SP.with_hi(BytePos(13));
    let snippet = "\"left\\\nright\"";
    let body = cooked_literal_body(snippet).expect("snippet is one cooked literal");
    let join = body
        .scan()
        .into_iter()
        .next()
        .expect("literal contains one join");
    let mapped = body_range_span(
        literal,
        body.len(),
        join.escape_range.start.get()..join.escape_range.end.get(),
    );

    assert!(mapped.is_some());
    assert!(preserves_value(body.as_str(), &["left", "right"]));
}

#[test]
fn corrupted_fragment_output_is_suppressed_by_the_value_gate() {
    assert!(!preserves_value("left\\\nright", &["left", "wrnog"]));
}
