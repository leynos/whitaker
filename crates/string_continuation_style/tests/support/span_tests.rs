//! Verifies quote adjustment and fail-closed span bounds.

use rustc_span::{DUMMY_SP, InnerSpan};

use super::{TokenOffset, body_range_span};

#[test]
fn body_offsets_start_after_the_opening_quote() {
    assert_eq!(TokenOffset::from_body_index(0, 4), Some(TokenOffset(1)));
    assert_eq!(TokenOffset::from_body_index(4, 4), Some(TokenOffset(5)));
}

#[test]
fn body_offsets_outside_the_body_are_rejected() {
    assert_eq!(TokenOffset::from_body_index(5, 4), None);
    assert_eq!(TokenOffset::from_body_index(usize::MAX, usize::MAX), None);
}

#[test]
fn exact_token_range_maps_to_the_exact_body_span() {
    let token_span = DUMMY_SP.with_hi(rustc_span::BytePos(6));
    let actual = body_range_span(token_span, 4, 1..3);
    let expected = token_span.from_inner(InnerSpan::new(2, 4));

    assert_eq!(actual, Some(expected));
}

#[test]
fn malformed_ranges_and_token_lengths_fail_closed() {
    let token_span = DUMMY_SP.with_hi(rustc_span::BytePos(6));

    assert_eq!(body_range_span(token_span, 4, 3..2), None);
    assert_eq!(body_range_span(token_span, 4, 0..5), None);
    assert_eq!(body_range_span(token_span, 3, 0..1), None);
}
