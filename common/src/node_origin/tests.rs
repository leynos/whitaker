//! Tests for the compiler-free provenance policy.

use proptest::prelude::*;
use rstest::rstest;

use super::{ExpansionMark, NodeOrigin, classify};

const ALL_MARKS: [ExpansionMark; 3] = [
    ExpansionMark::Desugaring,
    ExpansionMark::Macro,
    ExpansionMark::AstPass,
];

/// Specify authored origin as exactly the all-desugaring sequences.
///
/// This is the independent oracle: authored exactly when every mark is a
/// desugaring, including the empty sequence.
fn specified_origin(marks: &[ExpansionMark]) -> NodeOrigin {
    if marks.iter().all(|mark| *mark == ExpansionMark::Desugaring) {
        NodeOrigin::Authored
    } else {
        NodeOrigin::Generated
    }
}

/// Enumerate every mark sequence of exactly `length`, in lexicographic order.
fn sequences_of_length(length: usize) -> Vec<Vec<ExpansionMark>> {
    let mut sequences = vec![Vec::new()];
    for _ in 0..length {
        let mut expanded = Vec::with_capacity(sequences.len() * ALL_MARKS.len());
        for prefix in &sequences {
            for mark in ALL_MARKS {
                let mut sequence = prefix.clone();
                sequence.push(mark);
                expanded.push(sequence);
            }
        }
        sequences = expanded;
    }
    sequences
}

/// Check the explicit short sequences against their expected origins.
#[rstest]
#[case::empty(vec![], NodeOrigin::Authored)]
#[case::desugaring(vec![ExpansionMark::Desugaring], NodeOrigin::Authored)]
#[case::macro_mark(vec![ExpansionMark::Macro], NodeOrigin::Generated)]
#[case::ast_pass(vec![ExpansionMark::AstPass], NodeOrigin::Generated)]
#[case::desugaring_then_macro(
    vec![ExpansionMark::Desugaring, ExpansionMark::Macro],
    NodeOrigin::Generated
)]
#[case::macro_then_desugaring(
    vec![ExpansionMark::Macro, ExpansionMark::Desugaring],
    NodeOrigin::Generated
)]
#[case::two_desugarings(
    vec![ExpansionMark::Desugaring, ExpansionMark::Desugaring],
    NodeOrigin::Authored
)]
#[case::two_macros(
    vec![ExpansionMark::Macro, ExpansionMark::Macro],
    NodeOrigin::Generated
)]
#[case::desugaring_then_ast_pass(
    vec![ExpansionMark::Desugaring, ExpansionMark::AstPass],
    NodeOrigin::Generated
)]
#[case::ast_pass_then_desugaring(
    vec![ExpansionMark::AstPass, ExpansionMark::Desugaring],
    NodeOrigin::Generated
)]
#[case::desugaring_desugaring_macro(
    vec![
        ExpansionMark::Desugaring,
        ExpansionMark::Desugaring,
        ExpansionMark::Macro,
    ],
    NodeOrigin::Generated
)]
#[case::macro_desugaring_macro(
    vec![
        ExpansionMark::Macro,
        ExpansionMark::Desugaring,
        ExpansionMark::Macro,
    ],
    NodeOrigin::Generated
)]
fn explicit_sequences_classify(#[case] marks: Vec<ExpansionMark>, #[case] expected: NodeOrigin) {
    assert_eq!(classify(marks), expected);
}

/// Check every sequence of length zero to four against the spec oracle.
#[test]
fn exhaustive_lengths_zero_to_four_cover_121_sequences() {
    let sequences: Vec<_> = (0..=4).flat_map(sequences_of_length).collect();

    assert_eq!(sequences.len(), 121);
    for marks in sequences {
        assert_eq!(
            classify(marks.iter().copied()),
            specified_origin(&marks),
            "unexpected origin for {marks:?}"
        );
    }
}

/// Generate any of the three expansion marks.
fn mark_strategy() -> impl Strategy<Value = ExpansionMark> {
    prop_oneof![
        Just(ExpansionMark::Desugaring),
        Just(ExpansionMark::Macro),
        Just(ExpansionMark::AstPass),
    ]
}

proptest! {
    /// Check that lengths 0 through 256 agree with the existential specification.
    #[test]
    fn classify_matches_the_existential_oracle(
        marks in prop::collection::vec(mark_strategy(), 0..=256_usize)
    ) {
        prop_assert_eq!(classify(marks.iter().copied()), specified_origin(&marks));
    }

    /// Check that inserting a desugaring anywhere leaves the origin unchanged.
    #[test]
    fn inserting_a_desugaring_preserves_origin(
        marks in prop::collection::vec(mark_strategy(), 0..=256_usize),
        position in 0..=256_usize,
        generated in prop_oneof![
            Just(ExpansionMark::Macro),
            Just(ExpansionMark::AstPass),
        ],
    ) {
        let expected = specified_origin(&marks);
        let insert_at = position.min(marks.len());
        let mut with_desugaring = marks.clone();
        with_desugaring.insert(insert_at, ExpansionMark::Desugaring);
        prop_assert_eq!(classify(with_desugaring), expected);

        // A generated mark at the same position must override any desugaring.
        let mut with_generated = marks;
        with_generated.insert(insert_at, generated);
        prop_assert_eq!(classify(with_generated), NodeOrigin::Generated);
    }

    /// Check that concatenation is generated exactly when either component is generated.
    #[test]
    fn concatenation_is_generated_when_either_part_is(
        prefix in prop::collection::vec(mark_strategy(), 0..=128_usize),
        suffix in prop::collection::vec(mark_strategy(), 0..=128_usize),
    ) {
        let joined: Vec<_> = prefix.iter().chain(&suffix).copied().collect();
        let expected = if specified_origin(&prefix) == NodeOrigin::Generated
            || specified_origin(&suffix) == NodeOrigin::Generated
        {
            NodeOrigin::Generated
        } else {
            NodeOrigin::Authored
        };

        prop_assert_eq!(classify(joined), expected);
    }
}
