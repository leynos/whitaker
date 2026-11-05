//! Property tests for continuation ranges and consumed whitespace.

use proptest::prelude::*;

use super::Continuation;
use crate::{
    facts::{BodyOffset, LiteralBody},
    props::body_with_continuations,
};

/// The fragment alphabet this suite generates.
///
/// Deliberately narrower than the rewrite suite's: range checking needs
/// multi-byte characters and escaped backslashes, but not the unicodescape or
/// emoji that only matter once the value is decoded.
const ALPHABET: &[&str] = &["alpha", "λ", "\\\"", "\\\\", "{{", "}}"];

fn assert_scan_well_formed(
    body: &str,
    continuations: &[Continuation],
) -> Result<(), TestCaseError> {
    let body_end = BodyOffset::from_index(body.len());
    let mut previous_end = BodyOffset::from_index(0);

    for continuation in continuations {
        prop_assert!(continuation.escape_range.start < continuation.escape_range.end);
        prop_assert!(continuation.escape_range.end <= continuation.skipped_whitespace_range.start);
        prop_assert!(
            continuation.skipped_whitespace_range.start
                <= continuation.skipped_whitespace_range.end
        );
        prop_assert!(continuation.skipped_whitespace_range.end <= body_end);
        prop_assert!(previous_end <= continuation.escape_range.start);

        let skipped = body.as_bytes().get(
            continuation.skipped_whitespace_range.start.get()
                ..continuation.skipped_whitespace_range.end.get(),
        );
        let skipped_is_whitespace = skipped.is_some_and(|bytes| {
            bytes
                .iter()
                .all(|byte| super::is_consumed_whitespace(*byte))
        });
        prop_assert!(skipped_is_whitespace);

        previous_end = continuation.skipped_whitespace_range.end;
    }

    Ok(())
}

proptest! {
    #[test]
    fn constructed_bodies_have_ordered_non_overlapping_ranges(
        (body, expected_count) in body_with_continuations(ALPHABET)
    ) {
        let continuations = LiteralBody::new(&body).scan();
        prop_assert!(!continuations.is_empty());
        prop_assert_eq!(continuations.len(), expected_count);
        assert_scan_well_formed(&body, &continuations)?;
    }

    #[test]
    fn every_generated_continuation_consumes_indented_whitespace(
        (body, expected_count) in body_with_continuations(ALPHABET)
    ) {
        let continuations = LiteralBody::new(&body).scan();
        prop_assert!(!continuations.is_empty());
        prop_assert_eq!(continuations.len(), expected_count);
        for continuation in &continuations {
            prop_assert!(
                continuation.skipped_whitespace_range.start
                    < continuation.skipped_whitespace_range.end
            );
        }
        assert_scan_well_formed(&body, &continuations)?;
    }
}
