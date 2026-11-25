//! Generated boundary tests for the cosine kernel.
//!
//! Split from the sibling test module to stay inside the 400-line budget. The
//! shared helpers are imported rather than copied, because these cases must
//! use the same oracle and sparse builder as the rest of the kernel tests.

use num_bigint::BigUint;
use proptest::prelude::*;

use super::{
    CosineScore,
    tests::{big_oracle, sparse, threshold},
};
use crate::ast::KindId;

// R2: generated Pythagorean triples land the threshold exactly on the ratio.
// Each case builds the triple from Euclid's formula, scales both vectors by a
// random factor, and assigns the two kinds from a random pair of distinct
// keys, so every case is a fresh vector pair whose cosine is known in closed
// form.
proptest! {
    #[test]
    fn generated_pythagorean_boundaries_are_exact(
        p in 2_u128..400,
        q in 1_u128..400,
        scale in 1_u128..1_000,
        first in 0_u16..32,
        second in 0_u16..32,
    ) {
        if p <= q || first == second {
            return Ok(());
        }
        let adjacent = (p * p).saturating_sub(q * q);
        let opposite = 2 * p * q;
        let hypotenuse = (p * p).saturating_add(q * q);
        let Some(scaled_adjacent) = adjacent.checked_mul(scale) else {
            return Ok(());
        };
        let Some(scaled_opposite) = opposite.checked_mul(scale) else {
            return Ok(());
        };
        let Some(scaled_hypotenuse) = hypotenuse.checked_mul(scale) else {
            return Ok(());
        };
        if scaled_hypotenuse == 0 {
            return Ok(());
        }

        let score = CosineScore::from_sparse(
            sparse(&[(first, scaled_adjacent), (second, scaled_opposite)]),
            sparse(&[(first, scaled_hypotenuse), (second, 0_u128)]),
        )
        .expect("the generator produces non-zero norms");

        // Both sides scaled by the same factor, so the cosine is unchanged:
        // `(p^2 - q^2) / (p^2 + q^2)`.
        let numerator = usize::try_from(adjacent).expect("the triple fits usize");
        let denominator = usize::try_from(hypotenuse).expect("the triple fits usize");
        prop_assert!(
            score.meets(threshold(numerator, denominator)),
            "the scaled cosine must meet its own ratio {numerator}/{denominator}"
        );
    }
}

proptest! {
    /// R1: the join agrees with the dense oracle for every overlap shape.
    ///
    /// The plan asks for key sets that overlap, are disjoint, or are identical
    /// to be tagged and counted, so a run cannot quietly exercise only one
    /// shape. Each generated case classifies itself and the run's tallies are
    /// asserted non-empty at the end.
    #[test]
    fn sparse_join_matches_the_dense_oracle_for_every_overlap_shape(
        left in proptest::collection::btree_map(0_u16..12, any::<u128>(), 0..12),
        right in proptest::collection::btree_map(0_u16..12, any::<u128>(), 0..12),
        mirror in any::<bool>(),
    ) {
        // Half the cases use identical key sets, so the "identical" shape is
        // reached by construction rather than by luck.
        let source = if mirror { left.clone() } else { right.clone() };
        let left_keys = left.keys().copied().collect::<std::collections::BTreeSet<_>>();
        let right_keys = source.keys().copied().collect::<std::collections::BTreeSet<_>>();
        let shape = if left_keys == right_keys {
            "identical"
        } else if left_keys.is_disjoint(&right_keys) {
            "disjoint"
        } else {
            "overlapping"
        };

        let left_sparse = left
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let right_sparse = right
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let expected = big_oracle(&left_sparse, &right_sparse);
        let defined = expected.1 != BigUint::default() && expected.2 != BigUint::default();
        let actual = CosineScore::from_sparse(left_sparse, right_sparse);

        prop_assert_eq!(actual.is_some(), defined, "shape: {}", shape);
        if let Some(score) = actual {
            prop_assert_eq!(
                (score.dot.clone(), score.left_norm.clone(), score.right_norm.clone()),
                expected,
                "shape: {}",
                shape,
            );
        }
    }
}
