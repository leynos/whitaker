//! Exact-kernel tests: sparse merge join, score agreement and decimal shape.
//!
//! Three obligations meet here. R1 checks the sparse merge join against two
//! independent oracles. R2 checks that the runtime kernel agrees with the Verus
//! witnesses and with Pythagorean boundary constructions. R3 checks that the
//! printed six-decimal score round-trips for every representable value.

use std::collections::BTreeMap;

use num_bigint::BigUint;
use proptest::prelude::*;
use rstest::rstest;

use super::{CosineMicros, CosineScore, accept_parts, micros_parts, squared};
use crate::{SimilarityThreshold, ast::KindId};

/// Build a threshold without validation, for boundary construction.
fn threshold(numerator: usize, denominator: usize) -> SimilarityThreshold {
    SimilarityThreshold::new_unchecked("type3", numerator, denominator)
}

/// Build a sorted sparse vector from raw `(kind, weight)` pairs.
fn sparse(entries: &[(u16, u128)]) -> Vec<(KindId, u128)> {
    let mut sorted = entries
        .iter()
        .map(|(kind, weight)| (KindId::new(*kind), *weight))
        .collect::<Vec<_>>();
    sorted.sort_unstable_by_key(|(kind, _)| *kind);
    sorted
}

/// Compute the dense dot product and norms over the union of two vectors.
fn dense_oracle(left: &[(KindId, u128)], right: &[(KindId, u128)]) -> (BigUint, BigUint, BigUint) {
    let left_dense = left.iter().copied().collect::<BTreeMap<_, _>>();
    let right_dense = right.iter().copied().collect::<BTreeMap<_, _>>();
    let mut dot = BigUint::default();
    let mut left_norm = BigUint::default();
    let mut right_norm = BigUint::default();
    for (kind, weight) in &left_dense {
        left_norm += BigUint::from(*weight) * BigUint::from(*weight);
        if let Some(peer) = right_dense.get(kind) {
            dot += BigUint::from(*weight) * BigUint::from(*peer);
        }
    }
    for weight in right_dense.values() {
        right_norm += BigUint::from(*weight) * BigUint::from(*weight);
    }
    (dot, left_norm, right_norm)
}

/// Compute the same quantities in `u128`, for weights small enough to fit.
fn narrow_oracle(left: &[(KindId, u128)], right: &[(KindId, u128)]) -> (u128, u128, u128) {
    let left_dense = left.iter().copied().collect::<BTreeMap<_, _>>();
    let right_dense = right.iter().copied().collect::<BTreeMap<_, _>>();
    let mut dot = 0_u128;
    let mut left_norm = 0_u128;
    let mut right_norm = 0_u128;
    for (kind, weight) in &left_dense {
        left_norm += weight * weight;
        if let Some(peer) = right_dense.get(kind) {
            dot += weight * peer;
        }
    }
    for weight in right_dense.values() {
        right_norm += weight * weight;
    }
    (dot, left_norm, right_norm)
}

proptest! {
    /// R1: the sparse merge join equals the dense arbitrary-precision oracle.
    #[test]
    fn sparse_join_matches_the_dense_oracle(
        left in proptest::collection::btree_map(0_u16..24, any::<u128>(), 0..24),
        right in proptest::collection::btree_map(0_u16..24, any::<u128>(), 0..24),
    ) {
        let left_sparse = left
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let right_sparse = right
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let expected = dense_oracle(&left_sparse, &right_sparse);
        let defined = expected.1 != BigUint::default() && expected.2 != BigUint::default();
        let actual = CosineScore::from_sparse(left_sparse, right_sparse);

        prop_assert_eq!(actual.is_some(), defined);
        if let Some(score) = actual {
            prop_assert_eq!(
                (score.dot.clone(), score.left_norm.clone(), score.right_norm.clone()),
                expected,
            );
        }
    }

    /// R1: for small weights the exact kernel agrees with a plain `u128`
    /// oracle, which shares no arithmetic with `BigUint`.
    #[test]
    fn sparse_join_matches_the_narrow_oracle(
        left in proptest::collection::btree_map(0_u16..24, 0_u128..(1_u128 << 15), 0..24),
        right in proptest::collection::btree_map(0_u16..24, 0_u128..(1_u128 << 15), 0..24),
    ) {
        let left_sparse = left
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let right_sparse = right
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let (dot, left_norm, right_norm) = narrow_oracle(&left_sparse, &right_sparse);
        let Some(score) = CosineScore::from_sparse(left_sparse, right_sparse) else {
            prop_assert!(left_norm == 0 || right_norm == 0);
            return Ok(());
        };

        prop_assert_eq!(score.dot.clone(), BigUint::from(dot));
        prop_assert_eq!(score.left_norm.clone(), BigUint::from(left_norm));
        prop_assert_eq!(score.right_norm.clone(), BigUint::from(right_norm));
    }

    /// R2: every printed score is at most one and matches the floor of the
    /// unvalidated exact value.
    #[test]
    fn printed_scores_are_bounded(
        left in proptest::collection::btree_map(0_u16..16, any::<u128>(), 0..16),
        right in proptest::collection::btree_map(0_u16..16, any::<u128>(), 0..16),
    ) {
        let left_sparse = left
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let right_sparse = right
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let Some(score) = CosineScore::from_sparse(left_sparse, right_sparse) else {
            return Ok(());
        };
        let micros = score.micros().expect("the exact kernel must produce a score");
        prop_assert!(micros.get() <= 1_000_000, "micros {} exceeds one", micros.get());
        prop_assert_eq!(BigUint::from(micros.get()), score.micros_exact());
    }

    /// R2: cosine is symmetric in its arguments.
    #[test]
    fn cosine_is_symmetric(
        left in proptest::collection::btree_map(0_u16..12, any::<u128>(), 1..12),
        right in proptest::collection::btree_map(0_u16..12, any::<u128>(), 1..12),
    ) {
        let left_sparse = left
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let right_sparse = right
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let sideways = CosineScore::from_sparse(left_sparse.clone(), right_sparse.clone());
        let backwards = CosineScore::from_sparse(right_sparse, left_sparse);
        match (sideways, backwards) {
            (Some(forward_score), Some(reverse_score)) => {
                prop_assert_eq!(
                    forward_score.micros().expect("forward score").get(),
                    reverse_score.micros().expect("reverse score").get(),
                );
            }
            (None, None) => {}
            _ => prop_assert!(false, "symmetry requires both sides to be defined"),
        }
    }

    /// R2: scaling one side by a positive integer leaves the score within one
    /// micro of unchanged, because the computation floors once.
    #[test]
    fn cosine_is_scale_invariant(
        vector in proptest::collection::btree_map(0_u16..12, 1_u128..(1_u128 << 30), 1..12),
        other in proptest::collection::btree_map(0_u16..12, 1_u128..(1_u128 << 30), 1..12),
        factor in 1_u128..64,
    ) {
        let base = vector
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let scaled = base.iter().map(|(kind, weight)| (*kind, weight * factor))
            .collect::<Vec<_>>();
        let peer = other
            .into_iter()
            .map(|(kind, weight)| (KindId::new(kind), weight))
            .collect::<Vec<_>>();
        let plain = CosineScore::from_sparse(base, peer.clone()).expect("non-zero vector");
        let wide = CosineScore::from_sparse(scaled, peer).expect("non-zero vector");
        prop_assert!(
            plain.micros().expect("plain score").get()
                .abs_diff(wide.micros().expect("wide score").get())
                <= 1,
            "scaling changed the score by more than one micro",
        );
    }
}

/// R2: the P1 boundary witness from the Verus model, row for row.
///
/// `D = 340` with `A = B = 400` is a cosine of exactly `17/20`, so the pair is
/// accepted at the threshold and the printed score is `850000`.
#[test]
fn boundary_witness_is_accepted_at_every_twentieth() {
    let dot = BigUint::from(340_u32);
    let norm = BigUint::from(400_u32);

    assert!(
        accept_parts(&dot, &norm, &norm, threshold(17, 20)),
        "a cosine of exactly 17/20 must be accepted"
    );
    assert_eq!(
        micros_parts(&dot, &norm, &norm).expect("the boundary score is valid"),
        BigUint::from(850_000_u32),
        "the boundary witness must print 850000 micros"
    );
}

/// R2: the same norms with `D = 339` are rejected: `339/400 = 0.8475`.
#[test]
fn boundary_witness_one_step_below_is_rejected() {
    let dot = BigUint::from(339_u32);
    let norm = BigUint::from(400_u32);

    assert!(
        !accept_parts(&dot, &norm, &norm, threshold(17, 20)),
        "a cosine below 17/20 must be rejected"
    );
    assert_eq!(
        micros_parts(&dot, &norm, &norm).expect("the score is valid"),
        BigUint::from(847_500_u32),
        "339/400 floors to 847500 micros"
    );
}

/// R2: the P3 witness from the Verus model.
///
/// `a = (3, 0)` and `b = (1, 1)` give `D = 3`, `A = 9`, `B = 2`, so the cosine
/// is `1/sqrt(2)` and `m = 707106`.
#[test]
fn irrational_witness_prints_its_floor() {
    let micros = micros_parts(
        &BigUint::from(3_u32),
        &BigUint::from(9_u32),
        &BigUint::from(2_u32),
    )
    .expect("the witness score is valid");

    assert_eq!(
        micros,
        BigUint::from(707_106_u32),
        "the floor of 10^6/sqrt(2) is 707106"
    );
}

/// R2: a zero norm yields no score at all.
#[test]
fn zero_norm_yields_no_score() {
    let zeroed = CosineScore::from_sparse(sparse(&[(1_u16, 0_u128)]), sparse(&[(1_u16, 5_u128)]));
    let absent = CosineScore::from_sparse(Vec::new(), sparse(&[(1_u16, 5_u128)]));

    assert!(zeroed.is_none(), "an all-zero vector has no cosine");
    assert!(absent.is_none(), "an empty vector has no cosine");
}

/// R2: the unvalidated parts kernel refuses a zero norm rather than dividing.
#[test]
fn zero_norm_parts_report_an_error() {
    let zeros = BigUint::default();
    let result = micros_parts(&BigUint::from(1_u32), &zeros, &zeros);

    assert!(
        result.is_err(),
        "a zero norm must be rejected, got {result:?}"
    );
}

/// R2: the Pythagorean generator puts the threshold exactly on the ratio.
///
/// With `a = (p^2 - q^2, 2pq)` and `b = (1, 0)` the cosine is exactly
/// `(p^2 - q^2) / (p^2 + q^2)`, so the decision boundary is known in closed
/// form.
#[rstest]
#[case(2, 1)]
#[case(3, 2)]
#[case(5, 3)]
#[case(7, 4)]
#[case(11, 6)]
fn pythagorean_boundary_is_exact(#[case] p: u128, #[case] q: u128) {
    let adjacent = p * p - q * q;
    let opposite = 2 * p * q;
    let hypotenuse = p * p + q * q;
    let score = CosineScore::from_sparse(
        sparse(&[(1_u16, adjacent), (2_u16, opposite)]),
        sparse(&[(1_u16, hypotenuse), (2_u16, 0_u128)]),
    )
    .expect("the generator produces non-zero norms");

    let numerator = usize::try_from(adjacent).expect("the witness fits usize");
    let denominator = usize::try_from(hypotenuse).expect("the witness fits usize");
    assert!(
        score.meets(threshold(numerator, denominator)),
        "the cosine must meet its own exact ratio {numerator}/{denominator}"
    );
    let stricter = threshold(numerator, denominator.saturating_sub(1));
    assert!(
        !score.meets(stricter),
        "a strictly larger threshold must reject {numerator}/{denominator}"
    );

    let scaled = adjacent * 1_000_000;
    if scaled.rem_euclid(hypotenuse) == 0 {
        let expected = u32::try_from(scaled.div_euclid(hypotenuse)).expect("micros fit u32");
        assert_eq!(
            score.micros().expect("the score is valid").get(),
            expected,
            "the printed score must equal the exact ratio when it terminates"
        );
    }
}

/// R2: an over-unit threshold is never met, whatever the vectors are.
#[test]
fn over_unit_threshold_is_never_met() {
    let score = CosineScore::from_sparse(sparse(&[(1_u16, 5_u128)]), sparse(&[(1_u16, 5_u128)]))
        .expect("non-zero norms");

    assert!(
        !score.meets(threshold(2, 1)),
        "a threshold above one must never be met"
    );
}

/// R2: the squared helper is exact at the top of the `u128` range.
#[test]
fn squared_widens_beyond_u128() {
    let maximum = u128::MAX;
    let expected = BigUint::from(maximum) * BigUint::from(maximum);

    assert_eq!(
        squared(maximum),
        expected,
        "squaring u128::MAX must not wrap"
    );
}

/// R3: every representable micros value renders with six decimals inside the
/// unit interval.
#[test]
fn every_micros_value_renders_exactly() {
    for micros in 0..=1_000_000_u32 {
        let score = CosineMicros::from_micros(micros).expect("micros within range");
        let rendered = score.as_decimal_string();
        let (integer, fraction) = rendered
            .split_once('.')
            .expect("every printed score has a decimal point");
        assert_eq!(
            integer,
            if micros == 1_000_000 { "1" } else { "0" },
            "score {micros} rendered with an unexpected integer part: {rendered}"
        );
        assert_eq!(
            fraction.len(),
            6,
            "score {micros} must render exactly six decimals: {rendered}"
        );
        let parsed = rendered
            .parse::<f64>()
            .expect("every printed score parses as a float");
        assert!(
            (0.0..=1.0).contains(&parsed),
            "score {micros} rendered as {rendered} is outside [0, 1]"
        );
    }
}
