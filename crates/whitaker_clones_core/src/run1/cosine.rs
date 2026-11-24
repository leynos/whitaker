//! Exact cosine similarity over depth-weighted AST kind histograms.
//!
//! The acceptance decision must not depend on floating-point rounding, and the
//! operands exceed `u128`, so every product is computed exactly with
//! [`num_bigint::BigUint`]. The printed score is the cosine floored to six
//! decimals, rendered from the same exact integers.
//!
//! Acceptance mirrors the Verus model in `verus/clone_detector_type3_cosine.rs`
//! expression for expression: `d^2 * D^2 >= n^2 * A * B` decides
//! `cos(a, b) >= n/d`, where `D` is the dot product and `A`, `B` are the
//! squared norms. [`micros`](CosineScore::micros) additionally checks the
//! square-root brackets at runtime, so correctness never rests on
//! `BigUint::sqrt` being a floor square root.

use std::cmp::Ordering;

use num_bigint::BigUint;

use super::error::{Run1Error, Run1Result};
#[cfg(test)]
use crate::KindHistogram;
use crate::{ProductionMultiset, SimilarityThreshold};

/// Number of decimal places in the printed cosine score.
const MICROS_SCALE: u32 = 1_000_000;

/// Squared micros scale: `10^12 = (10^6)^2`, which makes the score exact.
const SCORE_SCALE: u64 = 1_000_000_000_000;

/// Cosine similarity floored to six decimal places.
///
/// The value lies in `0..=1_000_000`, where `1_000_000` means "cosine is
/// exactly one". It is the only score Whitaker prints, and it is derived from
/// the same exact integers that decide acceptance.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::CosineMicros;
///
/// let score = CosineMicros::from_micros(850_000).expect("a valid six-decimal score");
/// assert_eq!(score.get(), 850_000);
/// assert_eq!(score.as_decimal_string(), "0.850000");
/// assert!(CosineMicros::from_micros(1_000_001).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CosineMicros(u32);

impl CosineMicros {
    /// Builds a score from a six-decimal micros value.
    ///
    /// Returns `None` when `value` exceeds `1_000_000`, because a cosine
    /// cannot. The refinement pipeline produces its scores through the exact
    /// kernel rather than this constructor.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_clones_core::CosineMicros;
    ///
    /// assert_eq!(CosineMicros::from_micros(0).map(CosineMicros::get), Some(0));
    /// assert_eq!(
    ///     CosineMicros::from_micros(1_000_000).map(CosineMicros::get),
    ///     Some(1_000_000)
    /// );
    /// ```
    #[must_use]
    pub const fn from_micros(value: u32) -> Option<Self> {
        if value > MICROS_SCALE {
            return None;
        }
        Some(Self(value))
    }

    /// Returns the floored six-decimal score as an integer.
    #[must_use]
    pub const fn get(self) -> u32 { self.0 }

    /// Formats the score as a six-decimal string.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_clones_core::CosineMicros;
    ///
    /// let whole = CosineMicros::from_micros(1_000_000).expect("one is a valid cosine");
    /// let zero = CosineMicros::from_micros(0).expect("zero is a valid cosine");
    /// assert_eq!(whole.as_decimal_string(), "1.000000");
    /// assert_eq!(zero.as_decimal_string(), "0.000000");
    /// ```
    #[must_use]
    pub fn as_decimal_string(self) -> String {
        if self.0 == MICROS_SCALE {
            return "1.000000".to_owned();
        }
        format!("0.{:06}", self.0)
    }

    /// Builds a score from a verified micros value.
    ///
    /// # Errors
    ///
    /// Returns [`Run1Error::InvalidScore`] when `value` exceeds one.
    fn from_verified(value: u32) -> Run1Result<Self> {
        if value > MICROS_SCALE {
            return Err(invalid_score(value));
        }
        Ok(Self(value))
    }
}

/// Build an invalid-score error for a rejected intermediate value.
fn invalid_score(value: impl std::fmt::Display) -> Run1Error {
    Run1Error::InvalidScore {
        value: value.to_string(),
    }
}

/// Exact cosine inputs for one pair of sparse vectors.
///
/// Construct through [`from_sparse`](Self::from_sparse) or
/// [`between`](Self::between); both yield `None` when either vector is
/// all-zero, because the cosine is undefined there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CosineScore {
    dot: BigUint,
    left_norm: BigUint,
    right_norm: BigUint,
}

impl CosineScore {
    /// Computes the dot product and squared norms of two sorted sparse vectors.
    ///
    /// Each iterator must yield entries in ascending key order with positive
    /// weights; a missing key is a zero. Returns `None` when either squared
    /// norm is zero, because the cosine is then undefined.
    pub(crate) fn from_sparse<K, L, R>(left: L, right: R) -> Option<Self>
    where
        K: Ord + Copy,
        L: IntoIterator<Item = (K, u128)>,
        R: IntoIterator<Item = (K, u128)>,
    {
        let (dot, left_norm, right_norm) = merge_products(left.into_iter(), right.into_iter());
        if is_zero(&left_norm) || is_zero(&right_norm) {
            return None;
        }
        Some(Self {
            dot,
            left_norm,
            right_norm,
        })
    }

    /// Computes the score between two depth-weighted kind histograms.
    ///
    /// Retained for the calibration record: K1 measured this feature and
    /// rejected it, because unrelated functions score above 0.98.
    #[cfg(test)]
    pub(crate) fn between(left: &KindHistogram, right: &KindHistogram) -> Option<Self> {
        Self::from_sparse(
            left.iter().map(|(kind, weight)| (kind, weight.get())),
            right.iter().map(|(kind, weight)| (kind, weight.get())),
        )
    }

    /// Computes the score between two canonical trigram-production multisets.
    ///
    /// K1 selected this feature: it keeps statement shape while dropping the
    /// shallow "plumbing" kinds that made the weighted histogram score every
    /// pair above 0.98.
    pub(crate) fn between_productions(
        left: &ProductionMultiset,
        right: &ProductionMultiset,
    ) -> Option<Self> {
        Self::from_sparse(
            left.trigrams()
                .map(|(production, count)| (production, u128::from(count))),
            right
                .trigrams()
                .map(|(production, count)| (production, u128::from(count))),
        )
    }

    /// Reports whether the cosine is at least `threshold`.
    pub(crate) fn meets(&self, threshold: SimilarityThreshold) -> bool {
        accept_parts(&self.dot, &self.left_norm, &self.right_norm, threshold)
    }

    /// Returns the cosine floored to six decimals.
    ///
    /// # Errors
    ///
    /// Returns [`Run1Error::InvalidScore`] when the computed value falls
    /// outside its own square-root brackets or exceeds one.
    pub(crate) fn micros(&self) -> Run1Result<CosineMicros> {
        let exact = micros_parts(&self.dot, &self.left_norm, &self.right_norm)?;
        let narrowed = u32::try_from(&exact).map_err(|_| invalid_score(&exact))?;
        CosineMicros::from_verified(narrowed)
    }

    /// Returns the unvalidated floor of the exact micros value.
    ///
    /// Property tests use this to check the square-root brackets directly; the
    /// production path goes through [`micros`](Self::micros), which validates
    /// them.
    #[cfg(test)]
    pub(crate) fn micros_exact(&self) -> BigUint {
        let numerator = score_numerator(&self.dot);
        let denominator = &self.left_norm * &self.right_norm;
        (numerator / denominator).sqrt()
    }
}

/// Decide `D / (|a| |b|) >= n / d` by exact cross-multiplication.
///
/// Mirrors `accept` in `verus/clone_detector_type3_cosine.rs`.
fn accept_parts(
    dot: &BigUint,
    left_norm: &BigUint,
    right_norm: &BigUint,
    threshold: SimilarityThreshold,
) -> bool {
    if is_zero(left_norm) || is_zero(right_norm) {
        return false;
    }
    let denominator_squared = squared(usize_to_u128(threshold.denominator()));
    let numerator_squared = squared(usize_to_u128(threshold.numerator()));
    let left_side = denominator_squared * dot * dot;
    let right_side = numerator_squared * left_norm * right_norm;
    left_side >= right_side
}

/// Compute `floor(10^6 * cosine)` and verify its bracket.
///
/// With `x = 10^12 D^2` and `y = A B`, the score is `floor(sqrt(x / y))`. The
/// brackets `m^2 y <= x < (m + 1)^2 y` are checked at runtime rather than
/// assumed, so a defective square root cannot silently print a wrong score.
fn micros_parts(dot: &BigUint, left_norm: &BigUint, right_norm: &BigUint) -> Run1Result<BigUint> {
    if is_zero(left_norm) || is_zero(right_norm) {
        return Err(invalid_score("zero norm"));
    }
    let numerator = score_numerator(dot);
    let denominator = left_norm * right_norm;
    let candidate = (&numerator / &denominator).sqrt();
    if !bracket_holds(&candidate, &numerator, &denominator) {
        return Err(invalid_score(format_args!(
            "square-root bracket for {candidate}"
        )));
    }
    Ok(candidate)
}

/// Check `m^2 y <= x < (m + 1)^2 y` for a candidate floor square root.
fn bracket_holds(candidate: &BigUint, numerator: &BigUint, denominator: &BigUint) -> bool {
    let lower = squared_big(candidate) * denominator;
    if lower.cmp(numerator) == Ordering::Greater {
        return false;
    }
    let next = candidate + BigUint::from(1_u8);
    let upper = squared_big(&next) * denominator;
    numerator.cmp(&upper) == Ordering::Less
}

/// Compute `10^12 D^2`, the scaled squared numerator of the cosine.
fn score_numerator(dot: &BigUint) -> BigUint { BigUint::from(SCORE_SCALE) * dot * dot }

/// Square a non-negative integer of arbitrary precision.
fn squared_big(value: &BigUint) -> BigUint { value * value }

/// Square a `u128` exactly.
fn squared(value: u128) -> BigUint {
    let wide = BigUint::from(value);
    &wide * &wide
}

/// Widen a `usize` exactly.
///
/// `From<usize>` is not implemented for `u128`, so the cast is the only
/// lossless conversion available; `usize` is at most 64 bits wide.
const fn usize_to_u128(value: usize) -> u128 { value as u128 }

/// Report whether an unsigned big integer is zero.
fn is_zero(value: &BigUint) -> bool { *value == BigUint::default() }

/// Merge-join two ascending sparse vectors into `(dot, left_norm, right_norm)`.
fn merge_products<K, L, R>(left: L, right: R) -> (BigUint, BigUint, BigUint)
where
    K: Ord + Copy,
    L: Iterator<Item = (K, u128)>,
    R: Iterator<Item = (K, u128)>,
{
    let mut products = SparseProducts::default();
    let mut left_entries = left.peekable();
    let mut right_entries = right.peekable();

    while let (Some(left_entry), Some(right_entry)) =
        (left_entries.peek().copied(), right_entries.peek().copied())
    {
        match left_entry.0.cmp(&right_entry.0) {
            Ordering::Less => {
                left_entries.next();
                products.push_left(left_entry.1);
            }
            Ordering::Greater => {
                right_entries.next();
                products.push_right(right_entry.1);
            }
            Ordering::Equal => {
                left_entries.next();
                right_entries.next();
                products.push_pair(left_entry.1, right_entry.1);
            }
        }
    }
    products.extend_left(left_entries.by_ref());
    products.extend_right(right_entries.by_ref());
    (products.dot, products.left_norm, products.right_norm)
}

/// Partial products accumulated while merging two sparse vectors.
#[derive(Debug, Default)]
struct SparseProducts {
    dot: BigUint,
    left_norm: BigUint,
    right_norm: BigUint,
}

impl SparseProducts {
    /// Accumulate a weight present only in the left vector.
    fn push_left(&mut self, weight: u128) { self.left_norm += squared(weight); }

    /// Accumulate a weight present only in the right vector.
    fn push_right(&mut self, weight: u128) { self.right_norm += squared(weight); }

    /// Accumulate a weight present in both vectors.
    fn push_pair(&mut self, left: u128, right: u128) {
        self.dot += BigUint::from(left) * BigUint::from(right);
        self.left_norm += squared(left);
        self.right_norm += squared(right);
    }

    /// Accumulate every remaining left-only weight.
    fn extend_left<K: Ord + Copy>(&mut self, left: impl Iterator<Item = (K, u128)>) {
        for (_, weight) in left {
            self.push_left(weight);
        }
    }

    /// Accumulate every remaining right-only weight.
    fn extend_right<K: Ord + Copy>(&mut self, right: impl Iterator<Item = (K, u128)>) {
        for (_, weight) in right {
            self.push_right(weight);
        }
    }
}

#[cfg(test)]
#[path = "cosine_rendering_tests.rs"]
mod rendering_tests;
#[cfg(test)]
#[path = "cosine_tests.rs"]
mod tests;
