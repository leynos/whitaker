//! Deterministic MinHash sketch generation.

use std::{array, collections::BTreeSet};

use crate::token::Fingerprint;

use super::{IndexError, IndexResult, MINHASH_SIZE, MinHashSignature};

const SEED_STREAM_START: u64 = 0x243F_6A88_85A3_08D3;
const SEED_STREAM_STEP: u64 = 0x9E37_79B9_7F4A_7C15;
const HASH_MIX: u64 = 0x94D0_49BB_1331_11EB;

/// Deterministic MinHash sketcher for retained token fingerprints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinHasher {
    seeds: [u64; MINHASH_SIZE],
}

impl Default for MinHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl MinHasher {
    /// Creates the fixed 128-seed MinHash family used by roadmap item 7.2.2.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::{Fingerprint, MinHasher};
    ///
    /// let hasher = MinHasher::new();
    /// let signature = hasher.sketch(&[Fingerprint::new(7, 0..1)])?;
    /// assert_eq!(signature.values().len(), 128);
    /// # Ok::<(), whitaker_clones_core::IndexError>(())
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let mut state = SEED_STREAM_START;
        let seeds = array::from_fn(|_| next_seed(&mut state));
        Self { seeds }
    }

    /// Builds a MinHash sketch from retained fingerprint hashes.
    ///
    /// Duplicate fingerprint hash values are collapsed first so the sketch uses
    /// MinHash set semantics rather than multiset semantics.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::EmptyFingerprintSet`] when `fingerprints` is
    /// empty.
    pub fn sketch(&self, fingerprints: &[Fingerprint]) -> IndexResult<MinHashSignature> {
        let unique_hashes = unique_hashes(fingerprints)?;
        let values = array::from_fn(|index| minimum_mixed_hash(self.seeds[index], &unique_hashes));
        Ok(MinHashSignature::new(values))
    }
}

fn unique_hashes(fingerprints: &[Fingerprint]) -> IndexResult<BTreeSet<u64>> {
    if fingerprints.is_empty() {
        return Err(IndexError::EmptyFingerprintSet);
    }
    Ok(fingerprints
        .iter()
        .map(|fingerprint| fingerprint.hash)
        .collect())
}

fn minimum_mixed_hash(seed: u64, hashes: &BTreeSet<u64>) -> u64 {
    hashes
        .iter()
        .fold(u64::MAX, |current, hash| current.min(mix_hash(seed, *hash)))
}

fn mix_hash(seed: u64, hash: u64) -> u64 {
    splitmix64(seed ^ hash.wrapping_mul(HASH_MIX))
}

/// Generates the next seed in the deterministic stream.
///
/// Both `next_seed` and `splitmix64` intentionally add `SEED_STREAM_STEP` to
/// create a non-overlapping, deterministic seed sequence compatible with the
/// seed-streaming approach. This double-increment is deliberate, not a bug.
fn next_seed(state: &mut u64) -> u64 {
    *state = state.wrapping_add(SEED_STREAM_STEP);
    splitmix64(*state)
}

/// SplitMix64 generator with deliberate `SEED_STREAM_STEP` addition.
///
/// This function applies `SEED_STREAM_STEP` in addition to the increment in
/// `next_seed` to ensure deterministic, non-overlapping seed values.
fn splitmix64(value: u64) -> u64 {
    let mut mixed = value.wrapping_add(SEED_STREAM_STEP);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}
