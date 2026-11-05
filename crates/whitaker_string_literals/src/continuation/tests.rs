//! Boundary cases for continuation scanning.

use rstest::rstest;

use super::{Continuation, ContinuationKind};
use crate::facts::{BodyOffset, LiteralBody};

#[rstest]
#[case("alpha", 0)]
#[case("alpha\\\nbeta", 1)]
#[case("alpha\\\\\nbeta", 0)]
#[case("alpha\\\\\\\nbeta", 1)]
#[case("alpha\\\\\\\\\nbeta", 0)]
#[case("alpha\\\rbeta", 0)]
#[case("alpha\\\tbeta", 0)]
#[case("alpha\\", 0)]
fn detects_only_odd_backslash_runs_before_line_feed(#[case] body: &str, #[case] expected: usize) {
    let continuations = LiteralBody::new(body).scan();

    assert_eq!(continuations.len(), expected);
}

#[test]
fn escape_range_starts_at_the_unpaired_backslash() {
    let body = "alpha\\\\\\\nbeta";
    let continuations = LiteralBody::new(body).scan();
    let continuation = continuations
        .first()
        .expect("three backslashes before LF contain one continuation");

    assert_eq!(continuation.escape_range.start.get(), 7);
    assert_eq!(continuation.escape_range.end.get(), 9);
}

#[test]
fn consumes_mixed_ascii_whitespace_and_blank_lines_after_the_newline() {
    let body = "alpha\\\n \t\n\r  beta";
    let continuation = LiteralBody::new(body)
        .scan()
        .into_iter()
        .next()
        .expect("the odd slash run before LF is a continuation");

    assert_eq!(
        continuation.escape_range,
        BodyOffset::from_index(5)..BodyOffset::from_index(7)
    );
    assert_eq!(
        continuation.skipped_whitespace_range,
        BodyOffset::from_index(7)..BodyOffset::from_index(13)
    );
    assert_eq!(continuation.kind, ContinuationKind::Join);
}

#[test]
fn stops_before_formfeed_because_rust_does_not_consume_it() {
    let body = "alpha\\\n\u{000c}beta";
    let continuation = only_continuation(LiteralBody::new(body).scan());

    assert_eq!(
        continuation.skipped_whitespace_range.start,
        continuation.skipped_whitespace_range.end
    );
    assert_eq!(continuation.kind, ContinuationKind::Join);
}

#[rstest]
#[case(" \\\nbeta", ContinuationKind::LeadingLayoutTrim)]
#[case("alpha\\\n \n", ContinuationKind::TrailingLayoutTrim)]
#[case("\\\n \n", ContinuationKind::LeadingLayoutTrim)]
#[case("alpha\\\n beta", ContinuationKind::Join)]
fn classifies_content_around_a_continuation(
    #[case] body: &str,
    #[case] expected: ContinuationKind,
) {
    let continuation = only_continuation(LiteralBody::new(body).scan());

    assert_eq!(continuation.kind, expected);
}

fn only_continuation(continuations: Vec<Continuation>) -> Continuation {
    assert_eq!(continuations.len(), 1);
    continuations
        .into_iter()
        .next()
        .expect("the test body contains exactly one continuation")
}
