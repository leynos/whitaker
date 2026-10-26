//! Property tests for continuation ranges and consumed whitespace.

use proptest::prelude::*;

use super::Continuation;
use crate::facts::{BodyOffset, LiteralBody};

fn source_piece() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("alpha"),
            Just("λ"),
            Just("\\\""),
            Just("\\\\"),
            Just("{{"),
            Just("}}"),
        ],
        1..4,
    )
    .prop_map(|pieces| pieces.concat())
}

fn append_continuation(body: &mut String, whitespace: (u8, u8, u8)) {
    let (spaces, blank_lines, tabs) = whitespace;
    body.push('\\');
    body.push('\n');
    for _ in 0..spaces {
        body.push(' ');
    }
    for _ in 0..blank_lines {
        body.push('\n');
    }
    for _ in 0..tabs {
        body.push('\t');
    }
}

fn body_with_continuations() -> impl Strategy<Value = (String, usize)> {
    prop::collection::vec(source_piece(), 2..=6).prop_flat_map(|source_fragments| {
        let join_count = source_fragments.len().saturating_sub(1);
        (
            Just(source_fragments),
            prop::collection::vec((1_u8..4, 0_u8..3, 0_u8..3), join_count),
        )
            .prop_map(|(body_fragments, whitespace)| {
                let mut body = String::new();
                for (index, fragment) in body_fragments.iter().enumerate() {
                    body.push_str(fragment);
                    whitespace
                        .get(index)
                        .copied()
                        .into_iter()
                        .for_each(|settings| append_continuation(&mut body, settings));
                }
                let expected_continuations = body_fragments.len().saturating_sub(1);
                (body, expected_continuations)
            })
    })
}

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
        (body, expected_count) in body_with_continuations()
    ) {
        let continuations = LiteralBody::new(&body).scan();
        prop_assert!(!continuations.is_empty());
        prop_assert_eq!(continuations.len(), expected_count);
        assert_scan_well_formed(&body, &continuations)?;
    }

    #[test]
    fn every_generated_continuation_consumes_indented_whitespace(
        (body, expected_count) in body_with_continuations()
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
