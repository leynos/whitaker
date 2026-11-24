//! Kani harness for the Type-3 threshold-order comparison.
//!
//! The runtime comparison widens both `usize` operands to `u128` before
//! multiplying. The bound that justifies it is arithmetic, not empirical: for
//! any two `usize` values the 128-bit product is representable, because
//! `usize` is at most 64 bits. This harness checks that bound over the entire
//! domain, and that the widened comparison is asymmetric exactly when the two
//! fractions differ.

use super::classify::exceeds;
use crate::run0::SimilarityThreshold;

/// Compare two fractions with a checked widening, reporting whether both
/// products were representable.
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

/// Prove that the widened cross-multiplication never overflows and is
/// asymmetric exactly when the fractions differ.
#[kani::proof]
fn verify_threshold_order_matches_rational_order() {
    let high_numerator: usize = kani::any();
    let high_denominator: usize = kani::any();
    let low_numerator: usize = kani::any();
    let low_denominator: usize = kani::any();

    let higher = SimilarityThreshold::new_unchecked("higher", high_numerator, high_denominator);
    let lower = SimilarityThreshold::new_unchecked("lower", low_numerator, low_denominator);

    let (left, right) = checked_cross_products(higher, lower);
    assert!(
        left.is_some() && right.is_some(),
        "widening two usize operands to u128 must never overflow"
    );

    let (Some(left), Some(right)) = (left, right) else {
        return;
    };
    let widened = exceeds(higher, lower);
    assert!(
        widened == (left > right),
        "the widened comparison must match the checked cross-products"
    );
    assert!(
        widened != exceeds(lower, higher) || left == right,
        "the comparison must be asymmetric for unequal fractions"
    );
}
