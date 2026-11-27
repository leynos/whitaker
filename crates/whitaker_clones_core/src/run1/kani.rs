//! Widening contract for the Type-3 threshold-order comparison.
//!
//! The runtime comparison widens both `usize` operands to `u128` before
//! multiplying. The reason that cannot overflow is a fact about integer
//! widths, not about particular values: `usize` is at most 64 bits, so the
//! product of two of them needs at most 128 bits. [`WIDENING_IS_LOSSLESS`]
//! states that bound, and it holds at compile time over the whole domain.
//!
//! The remaining question — does the widened comparison compute the same thing
//! as an exactly-wide cross-multiplication? — is a `u128` multiplication
//! property. It is *not* checked by Kani here, and that is a recorded
//! limitation rather than an oversight: Kani 0.67's CBMC cannot discharge
//! symbolic 128-bit products at any domain size. A minimal probe of two 8-bit
//! symbolic operands multiplied as `u128` also missed a 600 s budget while
//! using under 400 MiB, so the cost is the bit-blast of the multiplier rather
//! than the value space; no bound or unwind adjustment brings it inside a
//! budget. An earlier version of this harness tried the full property and was
//! reported inconclusive by CI for exactly that reason.
//!
//! The property is carried by two other lanes, and the omission is reported by
//! `scripts/run-kani.sh` next to the verdicts so a green run never reads as a
//! proof:
//!
//! - `classify::tests::order_matches_the_rational_oracle` and `decimal_matches_the_rational_oracle`
//!   compare against an independent `BigUint` oracle over generated `usize` operands, including
//!   `usize::MAX`.
//! - `verus/clone_detector_type3_cosine.rs` proves the threshold arithmetic itself for arbitrary
//!   integers (P1 to P6).

#[cfg(test)]
use super::classify::exceeds;
#[cfg(test)]
use crate::run0::SimilarityThreshold;

/// Multiplying two widened `usize` values can never overflow `u128`.
///
/// This is the whole justification for the unchecked widening in [`exceeds`],
/// and it holds for every `usize` on every target `whitaker` supports. The
/// assertion is evaluated whenever this module is compiled, which is why the
/// constant exists rather than a bare `const _: ()`.
#[cfg(test)]
pub(super) const WIDENING_IS_LOSSLESS: () = assert!(
    usize::BITS * 2 <= u128::BITS,
    "two usize products must fit in a u128"
);

/// Compare two fractions with a checked widening, reporting whether both
/// products were representable.
#[cfg(test)]
fn checked_cross_products(
    higher: SimilarityThreshold,
    lower: SimilarityThreshold,
) -> (Option<u128>, Option<u128>) {
    let left = u128::try_from(higher.numerator())
        .ok()
        .and_then(|value| value.checked_mul(u128::try_from(lower.denominator()).ok()?));
    let right = u128::try_from(lower.numerator())
        .ok()
        .and_then(|value| value.checked_mul(u128::try_from(higher.denominator()).ok()?));
    (left, right)
}

/// Report whether the widened comparison agrees with the checked products.
#[cfg(test)]
pub(super) fn agrees(higher: SimilarityThreshold, lower: SimilarityThreshold) -> bool {
    let (checked_left, checked_right) = checked_cross_products(higher, lower);
    match (checked_left, checked_right) {
        (Some(lhs), Some(rhs)) => exceeds(higher, lower) == (lhs > rhs),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    //! Concrete checks of the widening contract, run as ordinary tests.
    //!
    //! These are the same statements the Kani harness cannot discharge
    //! symbolically, evaluated at the boundaries that matter: zero, one, the
    //! shipped defaults, the maximum, and operand orders that would expose an
    //! argument transposition.

    use rstest::rstest;

    use super::{WIDENING_IS_LOSSLESS, agrees};
    use crate::run0::SimilarityThreshold;

    /// Build a threshold without validation, so boundaries can be expressed.
    fn threshold(numerator: usize, denominator: usize) -> SimilarityThreshold {
        SimilarityThreshold::new_unchecked("probe", numerator, denominator)
    }

    /// Check that the static widening bound is part of the compiled crate.
    #[test]
    fn widening_bound_is_compiled_in() { WIDENING_IS_LOSSLESS }

    /// Check agreement at every interesting operand boundary.
    #[rstest]
    #[case(19, 20, 9, 10)]
    #[case(17, 20, 19, 20)]
    #[case(1, 1, 1, 1)]
    #[case(0, 1, 0, 1)]
    #[case(1, 2, 2, 4)]
    #[case(usize::MAX, usize::MAX, usize::MAX - 1, usize::MAX)]
    #[case(usize::MAX, 1, 1, usize::MAX)]
    #[case(9, usize::MAX, usize::MAX, 9)]
    fn the_widened_comparison_matches_checked_products(
        #[case] high_numerator: usize,
        #[case] high_denominator: usize,
        #[case] low_numerator: usize,
        #[case] low_denominator: usize,
    ) {
        let higher = threshold(high_numerator, high_denominator);
        let lower = threshold(low_numerator, low_denominator);

        assert!(
            agrees(higher, lower),
            "the widened comparison must equal the checked cross-products for \
             {high_numerator}/{high_denominator} against {low_numerator}/{low_denominator}"
        );
    }
}
