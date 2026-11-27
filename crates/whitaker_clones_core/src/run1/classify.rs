//! Threshold vocabulary and exact threshold validation for Run 1.
//!
//! The design requires `type1 > type2 > type3`, and the printed cosine must
//! never contradict the acceptance decision. The second requirement forces the
//! Type-3 threshold to be a six-decimal value: only then does
//! `micros >= 10^6 * type3` mean exactly `cosine >= type3`, which the Verus
//! proof P4 establishes.

use std::fmt;

use super::{
    config::AstPassConfig,
    error::{Run1Error, Run1Result},
    widen,
};
use crate::run0::{SimilarityThreshold, TokenPassConfig};

/// Six-decimal denominator used by the printed cosine score.
const MICROS_SCALE: u128 = 1_000_000;

/// The three similarity thresholds, in descending strength.
///
/// Only used to name the offending pair in [`Run1Error::ThresholdOrder`], so
/// the report can say which comparison failed.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ThresholdKind;
///
/// assert_eq!(ThresholdKind::Type3.to_string(), "type3");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThresholdKind {
    /// The Type-1 (token-exact) Jaccard threshold.
    Type1,
    /// The Type-2 (rename-equivalent) Jaccard threshold.
    Type2,
    /// The Type-3 (AST near-miss) cosine threshold.
    Type3,
}

impl fmt::Display for ThresholdKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Type1 => "type1",
            Self::Type2 => "type2",
            Self::Type3 => "type3",
        };
        formatter.write_str(name)
    }
}

/// Require `type1 > type2 > type3`.
///
/// # Errors
///
/// Returns [`Run1Error::ThresholdOrder`] naming the first adjacent pair that is
/// not strictly ordered.
pub const fn validate_threshold_order(
    type1: SimilarityThreshold,
    type2: SimilarityThreshold,
    type3: SimilarityThreshold,
) -> Run1Result<()> {
    if !exceeds(type1, type2) {
        return Err(Run1Error::ThresholdOrder {
            higher: ThresholdKind::Type1,
            lower: ThresholdKind::Type2,
        });
    }
    if !exceeds(type2, type3) {
        return Err(Run1Error::ThresholdOrder {
            higher: ThresholdKind::Type2,
            lower: ThresholdKind::Type3,
        });
    }
    Ok(())
}

/// Require the Type-3 threshold to be representable with six decimals.
///
/// # Errors
///
/// Returns [`Run1Error::ThresholdNotDecimal`] when `10^6 * numerator` is not a
/// multiple of `denominator`.
pub const fn validate_decimal_threshold(threshold: SimilarityThreshold) -> Run1Result<()> {
    if is_six_decimal(threshold) {
        return Ok(());
    }
    Err(Run1Error::not_decimal(threshold))
}

/// Report whether `10^6 * n` is exactly divisible by `d`.
const fn is_six_decimal(threshold: SimilarityThreshold) -> bool {
    let scaled = widen(threshold.numerator()) * MICROS_SCALE;
    let denominator = widen(threshold.denominator());
    matches!(scaled.checked_rem(denominator), Some(0))
}

/// Compare two thresholds as rationals by cross-multiplication.
///
/// Each operand is at most `usize::MAX`, so the widened products cannot
/// overflow `u128`.
pub(crate) const fn exceeds(higher: SimilarityThreshold, lower: SimilarityThreshold) -> bool {
    let left = widen(higher.numerator()) * widen(lower.denominator());
    let right = widen(lower.numerator()) * widen(higher.denominator());
    left > right
}
/// Validate the threshold configuration before any lowering work.
pub(super) fn validate_pass_config(
    token_config: &TokenPassConfig,
    config: &AstPassConfig,
) -> Run1Result<()> {
    token_config.type1_threshold().validate()?;
    token_config.type2_threshold().validate()?;
    config.type3_threshold().validate()?;
    validate_threshold_order(
        token_config.type1_threshold(),
        token_config.type2_threshold(),
        config.type3_threshold(),
    )?;
    validate_decimal_threshold(config.type3_threshold())
}

#[cfg(test)]
mod tests {
    //! Exhaustive threshold-validation tests and their `BigUint` oracle.

    use num_bigint::BigUint;
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    /// Build a threshold without validation, so malformed inputs can be tested.
    fn threshold(name: &'static str, numerator: usize, denominator: usize) -> SimilarityThreshold {
        SimilarityThreshold::new_unchecked(name, numerator, denominator)
    }

    /// Compare two rationals with an independent arbitrary-precision oracle.
    fn exceeds_by_biguint(higher: SimilarityThreshold, lower: SimilarityThreshold) -> bool {
        BigUint::from(higher.numerator()) * BigUint::from(lower.denominator())
            > BigUint::from(lower.numerator()) * BigUint::from(higher.denominator())
    }

    /// Report whether the oracle accepts a six-decimal Type-3 threshold.
    fn decimal_by_biguint(threshold: SimilarityThreshold) -> bool {
        if threshold.denominator() == 0 {
            return false;
        }
        let scaled = BigUint::from(threshold.numerator()) * BigUint::from(MICROS_SCALE);
        scaled % BigUint::from(threshold.denominator()) == BigUint::default()
    }

    /// Check the documented defaults are accepted.
    #[test]
    fn defaults_are_ordered() {
        let type1 = threshold("type1", 19, 20);
        let type2 = threshold("type2", 9, 10);
        let type3 = threshold("type3", 17, 20);
        let result = validate_threshold_order(type1, type2, type3);
        assert!(
            result.is_ok(),
            "the design defaults must be strictly ordered, got {result:?}"
        );
    }

    /// Check that equal neighbouring thresholds are rejected, naming the
    /// equal pair rather than a later comparison.
    #[rstest]
    #[case(
        (
            threshold("equal", 9, 10),
            threshold("equal", 9, 10),
            threshold("type3", 17, 20)
        ),
        (ThresholdKind::Type1, ThresholdKind::Type2)
    )]
    #[case(
        (
            threshold("type1", 19, 20),
            threshold("equal", 17, 20),
            threshold("equal", 17, 20)
        ),
        (ThresholdKind::Type2, ThresholdKind::Type3)
    )]
    fn equal_neighbours_are_rejected(
        #[case] thresholds: (
            SimilarityThreshold,
            SimilarityThreshold,
            SimilarityThreshold,
        ),
        #[case] expected: (ThresholdKind, ThresholdKind),
    ) {
        let (type1, type2, type3) = thresholds;
        let (expected_higher, expected_lower) = expected;
        let result = validate_threshold_order(type1, type2, type3);
        let Err(Run1Error::ThresholdOrder { higher, lower }) = result else {
            panic!("equal neighbouring thresholds must be rejected, got {result:?}");
        };
        assert_eq!(
            (higher, lower),
            (expected_higher, expected_lower),
            "the reported pair must name the equal neighbours"
        );
    }

    /// Check that `usize::MAX` operands compare without overflow.
    #[test]
    fn extreme_operands_compare_exactly() {
        let largest = threshold("largest", usize::MAX, usize::MAX);
        let smaller = threshold("smaller", usize::MAX - 1, usize::MAX);
        let type3 = threshold("type3", 1, usize::MAX);
        let result = validate_threshold_order(largest, smaller, type3);
        assert!(
            result.is_ok(),
            "u128 cross-multiplication must not overflow, got {result:?}"
        );
    }

    /// Check the decimal restriction against the documented examples.
    #[rstest]
    #[case(17, 20, true)]
    #[case(1, 1, true)]
    #[case(1, 3, false)]
    #[case(2, 3, false)]
    #[case(1, 1_000_000, true)]
    #[case(1, 3_000_000, false)]
    fn six_decimal_thresholds(
        #[case] numerator: usize,
        #[case] denominator: usize,
        #[case] ok: bool,
    ) {
        let result = validate_decimal_threshold(threshold("type3", numerator, denominator));
        assert_eq!(
            result.is_ok(),
            ok,
            "unexpected decimal verdict for {numerator}/{denominator}: {result:?}"
        );
    }

    proptest! {
        /// Check the widened comparison against the arbitrary-precision oracle.
        #[test]
        fn order_matches_the_rational_oracle(
            high_numerator in any::<usize>(),
            high_denominator in any::<usize>(),
            low_numerator in any::<usize>(),
            low_denominator in any::<usize>(),
        ) {
            let higher = threshold("higher", high_numerator, high_denominator);
            let lower = threshold("lower", low_numerator, low_denominator);
            prop_assert_eq!(exceeds(higher, lower), exceeds_by_biguint(higher, lower));
        }

        /// Check the decimal restriction against the arbitrary-precision oracle.
        #[test]
        fn decimal_matches_the_rational_oracle(
            numerator in any::<usize>(),
            denominator in any::<usize>(),
        ) {
            let threshold = threshold("type3", numerator, denominator);
            prop_assert_eq!(is_six_decimal(threshold), decimal_by_biguint(threshold));
        }
    }
}
