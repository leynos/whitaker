//! Integer-only token-pass similarity scoring helpers.

use std::collections::BTreeSet;

use crate::{Fingerprint, NormProfile};

use super::error::{Run0Error, Run0Result};

/// Integer-backed Jaccard similarity ratio.
///
/// The score represents `intersection / union` using set semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimilarityRatio {
    intersection: usize,
    union: usize,
}

impl SimilarityRatio {
    /// Creates a ratio from raw set counts.
    #[must_use]
    pub const fn new(intersection: usize, union: usize) -> Self {
        Self {
            intersection,
            union,
        }
    }

    /// Returns the numerator of the ratio.
    #[must_use]
    pub const fn intersection(self) -> usize {
        self.intersection
    }

    /// Returns the denominator of the ratio.
    #[must_use]
    pub const fn union(self) -> usize {
        self.union
    }

    /// Formats the ratio as a six-decimal string without floating-point arithmetic.
    #[must_use]
    pub fn as_decimal_string(self) -> String {
        let Some((integer, remainder)) = repeated_division(self.intersection, self.union) else {
            return "0.000000".to_owned();
        };

        let mut decimals = String::new();
        let mut current = remainder;
        for _ in 0..6 {
            current = current.saturating_mul(10);
            let (digit, next_remainder) = repeated_division(current, self.union).unwrap_or((0, 0));
            decimals.push(char::from(
                b'0'.saturating_add(u8::try_from(digit).unwrap_or(0)),
            ));
            current = next_remainder;
        }

        format!("{integer}.{decimals}")
    }
}

/// Integer-backed threshold for a similarity ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimilarityThreshold {
    name: &'static str,
    numerator: usize,
    denominator: usize,
}

impl SimilarityThreshold {
    /// Validates and creates a new threshold.
    ///
    /// # Errors
    ///
    /// Returns [`Run0Error::InvalidThreshold`] if the threshold is not within `(0, 1]`.
    pub fn new(name: &'static str, numerator: usize, denominator: usize) -> Run0Result<Self> {
        let threshold = Self::new_unchecked(name, numerator, denominator);
        threshold.validate()?;
        Ok(threshold)
    }

    /// Creates a threshold without validation for trusted internal defaults.
    #[must_use]
    pub const fn new_unchecked(name: &'static str, numerator: usize, denominator: usize) -> Self {
        Self {
            name,
            numerator,
            denominator,
        }
    }

    /// Returns the threshold numerator.
    #[must_use]
    pub const fn numerator(self) -> usize {
        self.numerator
    }

    /// Returns the threshold denominator.
    #[must_use]
    pub const fn denominator(self) -> usize {
        self.denominator
    }

    /// Returns `true` if the threshold represents a ratio within `(0, 1]`.
    fn is_valid(self) -> bool {
        self.numerator != 0 && self.denominator != 0 && self.numerator <= self.denominator
    }

    pub(crate) fn validate(self) -> Run0Result<()> {
        if !self.is_valid() {
            return Err(Run0Error::InvalidThreshold {
                name: self.name.to_owned(),
            });
        }
        Ok(())
    }
}

pub(crate) fn select_rule_profile(
    profile: NormProfile,
    score: SimilarityRatio,
    type1_threshold: SimilarityThreshold,
    type2_threshold: SimilarityThreshold,
) -> Option<NormProfile> {
    match profile {
        NormProfile::T1 if meets_threshold(score, type1_threshold) => Some(NormProfile::T1),
        NormProfile::T2 if meets_threshold(score, type2_threshold) => Some(NormProfile::T2),
        _ => None,
    }
}

pub(crate) fn jaccard_similarity(
    left: &[Fingerprint],
    right: &[Fingerprint],
) -> Option<SimilarityRatio> {
    let left_hashes = unique_hashes(left);
    let right_hashes = unique_hashes(right);
    if left_hashes.is_empty() || right_hashes.is_empty() {
        return None;
    }

    let intersection = left_hashes.intersection(&right_hashes).count();
    let union = left_hashes.union(&right_hashes).count();
    Some(SimilarityRatio::new(intersection, union))
}

fn unique_hashes(fingerprints: &[Fingerprint]) -> BTreeSet<u64> {
    fingerprints
        .iter()
        .map(|fingerprint| fingerprint.hash)
        .collect()
}

fn meets_threshold(score: SimilarityRatio, threshold: SimilarityThreshold) -> bool {
    score.intersection.saturating_mul(threshold.denominator)
        >= score.union.saturating_mul(threshold.numerator)
}

fn repeated_division(numerator: usize, denominator: usize) -> Option<(usize, usize)> {
    debug_assert!(denominator != 0, "denominator must be non-zero");
    if denominator == 0 {
        return None;
    }

    let quotient = numerator / denominator;
    let remainder = numerator % denominator;

    Some((quotient, remainder))
}
