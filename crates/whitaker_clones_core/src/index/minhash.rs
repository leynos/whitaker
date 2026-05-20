//! Deterministic MinHash sketch generation.

use std::array;

use crate::token::Fingerprint;

use super::{IndexError, IndexResult, MINHASH_SIZE, MinHashSignature};

const SEED_STREAM_START: u64 = 0x243F_6A88_85A3_08D3;
const SEED_STREAM_STEP: u64 = 0x9E37_79B9_7F4A_7C15;
const HASH_MIX: u64 = 0x94D0_49BB_1331_11EB;

/// A typed MinHash seed value.
///
/// Keeps seed values distinct from raw hash values at the type level,
/// preventing accidental argument transposition inside the hashing core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seed(u64);

/// Deterministic MinHash sketcher for retained token fingerprints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinHasher {
    seeds: [Seed; MINHASH_SIZE],
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
        let values = sketch_values(&self.seeds, &unique_hashes);
        Ok(MinHashSignature::new(values))
    }

    /// Creates a deterministic proof-only fixture for Kani harnesses.
    ///
    /// This `#[cfg(kani)]` proof seam fills all [`MINHASH_SIZE`] seeds with the
    /// same value, avoiding seed-stream construction to drastically reduce
    /// Kani's symbolic state space during bounded model checking. Production
    /// code continues to use [`MinHasher::new`]; this constructor exists only
    /// to provide deterministic, lightweight inputs for Kani proofs rather than
    /// production logic.
    #[cfg(kani)]
    pub(super) fn from_seed_for_kani(seed: u64) -> Self {
        Self {
            seeds: [Seed(seed); MINHASH_SIZE],
        }
    }

    #[cfg(kani)]
    pub(super) fn from_checked_lane_seeds_for_kani(
        first_seed: u64,
        middle_seed: u64,
        last_seed: u64,
    ) -> Self {
        let mut seeds = [Seed(first_seed); MINHASH_SIZE];
        seeds[MINHASH_SIZE / 2] = Seed(middle_seed);
        seeds[MINHASH_SIZE - 1] = Seed(last_seed);
        Self { seeds }
    }
}

/// Computes the 128-lane MinHash signature in production builds.
///
/// Two `cfg`-specific implementations exist to balance production idiom
/// against proof tractability. This production variant uses idiomatic
/// [`array::from_fn`] iteration. It is mechanically equivalent to the
/// `#[cfg(kani)]` proof-seam variant: both call [`minimum_mixed_hash`] once per
/// lane to produce the same 128-lane MinHash signature from the seeds and
/// unique hashes.
#[cfg(not(kani))]
fn sketch_values(seeds: &[Seed; MINHASH_SIZE], unique_hashes: &[u64]) -> [u64; MINHASH_SIZE] {
    array::from_fn(|index| minimum_mixed_hash(seeds[index], unique_hashes))
}

/// Computes the 128-lane MinHash signature with explicit unrolling for Kani.
///
/// Two `cfg`-specific implementations exist to balance production idiom
/// against proof tractability. This `#[cfg(kani)]` proof seam manually unrolls
/// every [`minimum_mixed_hash`] call so Kani's bounded model checker does not
/// spend proof budget on iterator or loop state expansion. The explicit array
/// literal is mechanically equivalent to the production [`array::from_fn`]
/// implementation: both compute the same 128-lane MinHash signature from the
/// seeds and unique hashes.
#[cfg(kani)]
fn sketch_values(seeds: &[Seed; MINHASH_SIZE], unique_hashes: &[u64]) -> [u64; MINHASH_SIZE] {
    [
        minimum_mixed_hash(seeds[0], unique_hashes),
        minimum_mixed_hash(seeds[1], unique_hashes),
        minimum_mixed_hash(seeds[2], unique_hashes),
        minimum_mixed_hash(seeds[3], unique_hashes),
        minimum_mixed_hash(seeds[4], unique_hashes),
        minimum_mixed_hash(seeds[5], unique_hashes),
        minimum_mixed_hash(seeds[6], unique_hashes),
        minimum_mixed_hash(seeds[7], unique_hashes),
        minimum_mixed_hash(seeds[8], unique_hashes),
        minimum_mixed_hash(seeds[9], unique_hashes),
        minimum_mixed_hash(seeds[10], unique_hashes),
        minimum_mixed_hash(seeds[11], unique_hashes),
        minimum_mixed_hash(seeds[12], unique_hashes),
        minimum_mixed_hash(seeds[13], unique_hashes),
        minimum_mixed_hash(seeds[14], unique_hashes),
        minimum_mixed_hash(seeds[15], unique_hashes),
        minimum_mixed_hash(seeds[16], unique_hashes),
        minimum_mixed_hash(seeds[17], unique_hashes),
        minimum_mixed_hash(seeds[18], unique_hashes),
        minimum_mixed_hash(seeds[19], unique_hashes),
        minimum_mixed_hash(seeds[20], unique_hashes),
        minimum_mixed_hash(seeds[21], unique_hashes),
        minimum_mixed_hash(seeds[22], unique_hashes),
        minimum_mixed_hash(seeds[23], unique_hashes),
        minimum_mixed_hash(seeds[24], unique_hashes),
        minimum_mixed_hash(seeds[25], unique_hashes),
        minimum_mixed_hash(seeds[26], unique_hashes),
        minimum_mixed_hash(seeds[27], unique_hashes),
        minimum_mixed_hash(seeds[28], unique_hashes),
        minimum_mixed_hash(seeds[29], unique_hashes),
        minimum_mixed_hash(seeds[30], unique_hashes),
        minimum_mixed_hash(seeds[31], unique_hashes),
        minimum_mixed_hash(seeds[32], unique_hashes),
        minimum_mixed_hash(seeds[33], unique_hashes),
        minimum_mixed_hash(seeds[34], unique_hashes),
        minimum_mixed_hash(seeds[35], unique_hashes),
        minimum_mixed_hash(seeds[36], unique_hashes),
        minimum_mixed_hash(seeds[37], unique_hashes),
        minimum_mixed_hash(seeds[38], unique_hashes),
        minimum_mixed_hash(seeds[39], unique_hashes),
        minimum_mixed_hash(seeds[40], unique_hashes),
        minimum_mixed_hash(seeds[41], unique_hashes),
        minimum_mixed_hash(seeds[42], unique_hashes),
        minimum_mixed_hash(seeds[43], unique_hashes),
        minimum_mixed_hash(seeds[44], unique_hashes),
        minimum_mixed_hash(seeds[45], unique_hashes),
        minimum_mixed_hash(seeds[46], unique_hashes),
        minimum_mixed_hash(seeds[47], unique_hashes),
        minimum_mixed_hash(seeds[48], unique_hashes),
        minimum_mixed_hash(seeds[49], unique_hashes),
        minimum_mixed_hash(seeds[50], unique_hashes),
        minimum_mixed_hash(seeds[51], unique_hashes),
        minimum_mixed_hash(seeds[52], unique_hashes),
        minimum_mixed_hash(seeds[53], unique_hashes),
        minimum_mixed_hash(seeds[54], unique_hashes),
        minimum_mixed_hash(seeds[55], unique_hashes),
        minimum_mixed_hash(seeds[56], unique_hashes),
        minimum_mixed_hash(seeds[57], unique_hashes),
        minimum_mixed_hash(seeds[58], unique_hashes),
        minimum_mixed_hash(seeds[59], unique_hashes),
        minimum_mixed_hash(seeds[60], unique_hashes),
        minimum_mixed_hash(seeds[61], unique_hashes),
        minimum_mixed_hash(seeds[62], unique_hashes),
        minimum_mixed_hash(seeds[63], unique_hashes),
        minimum_mixed_hash(seeds[64], unique_hashes),
        minimum_mixed_hash(seeds[65], unique_hashes),
        minimum_mixed_hash(seeds[66], unique_hashes),
        minimum_mixed_hash(seeds[67], unique_hashes),
        minimum_mixed_hash(seeds[68], unique_hashes),
        minimum_mixed_hash(seeds[69], unique_hashes),
        minimum_mixed_hash(seeds[70], unique_hashes),
        minimum_mixed_hash(seeds[71], unique_hashes),
        minimum_mixed_hash(seeds[72], unique_hashes),
        minimum_mixed_hash(seeds[73], unique_hashes),
        minimum_mixed_hash(seeds[74], unique_hashes),
        minimum_mixed_hash(seeds[75], unique_hashes),
        minimum_mixed_hash(seeds[76], unique_hashes),
        minimum_mixed_hash(seeds[77], unique_hashes),
        minimum_mixed_hash(seeds[78], unique_hashes),
        minimum_mixed_hash(seeds[79], unique_hashes),
        minimum_mixed_hash(seeds[80], unique_hashes),
        minimum_mixed_hash(seeds[81], unique_hashes),
        minimum_mixed_hash(seeds[82], unique_hashes),
        minimum_mixed_hash(seeds[83], unique_hashes),
        minimum_mixed_hash(seeds[84], unique_hashes),
        minimum_mixed_hash(seeds[85], unique_hashes),
        minimum_mixed_hash(seeds[86], unique_hashes),
        minimum_mixed_hash(seeds[87], unique_hashes),
        minimum_mixed_hash(seeds[88], unique_hashes),
        minimum_mixed_hash(seeds[89], unique_hashes),
        minimum_mixed_hash(seeds[90], unique_hashes),
        minimum_mixed_hash(seeds[91], unique_hashes),
        minimum_mixed_hash(seeds[92], unique_hashes),
        minimum_mixed_hash(seeds[93], unique_hashes),
        minimum_mixed_hash(seeds[94], unique_hashes),
        minimum_mixed_hash(seeds[95], unique_hashes),
        minimum_mixed_hash(seeds[96], unique_hashes),
        minimum_mixed_hash(seeds[97], unique_hashes),
        minimum_mixed_hash(seeds[98], unique_hashes),
        minimum_mixed_hash(seeds[99], unique_hashes),
        minimum_mixed_hash(seeds[100], unique_hashes),
        minimum_mixed_hash(seeds[101], unique_hashes),
        minimum_mixed_hash(seeds[102], unique_hashes),
        minimum_mixed_hash(seeds[103], unique_hashes),
        minimum_mixed_hash(seeds[104], unique_hashes),
        minimum_mixed_hash(seeds[105], unique_hashes),
        minimum_mixed_hash(seeds[106], unique_hashes),
        minimum_mixed_hash(seeds[107], unique_hashes),
        minimum_mixed_hash(seeds[108], unique_hashes),
        minimum_mixed_hash(seeds[109], unique_hashes),
        minimum_mixed_hash(seeds[110], unique_hashes),
        minimum_mixed_hash(seeds[111], unique_hashes),
        minimum_mixed_hash(seeds[112], unique_hashes),
        minimum_mixed_hash(seeds[113], unique_hashes),
        minimum_mixed_hash(seeds[114], unique_hashes),
        minimum_mixed_hash(seeds[115], unique_hashes),
        minimum_mixed_hash(seeds[116], unique_hashes),
        minimum_mixed_hash(seeds[117], unique_hashes),
        minimum_mixed_hash(seeds[118], unique_hashes),
        minimum_mixed_hash(seeds[119], unique_hashes),
        minimum_mixed_hash(seeds[120], unique_hashes),
        minimum_mixed_hash(seeds[121], unique_hashes),
        minimum_mixed_hash(seeds[122], unique_hashes),
        minimum_mixed_hash(seeds[123], unique_hashes),
        minimum_mixed_hash(seeds[124], unique_hashes),
        minimum_mixed_hash(seeds[125], unique_hashes),
        minimum_mixed_hash(seeds[126], unique_hashes),
        minimum_mixed_hash(seeds[127], unique_hashes),
    ]
}

pub(super) fn unique_hashes(fingerprints: &[Fingerprint]) -> IndexResult<Vec<u64>> {
    if fingerprints.is_empty() {
        return Err(IndexError::EmptyFingerprintSet);
    }
    let mut hashes = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.hash)
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes.dedup();
    Ok(hashes)
}

fn minimum_mixed_hash(seed: Seed, hashes: &[u64]) -> u64 {
    hashes.iter().fold(u64::MAX, |current, hash| {
        current.min(mix_hash(seed.0, *hash))
    })
}

#[cfg(kani)]
pub(super) fn expected_lane_for_kani(seed: u64, hashes: &[u64]) -> u64 {
    minimum_mixed_hash(Seed(seed), hashes)
}

fn mix_hash(seed: u64, hash: u64) -> u64 {
    splitmix64(seed ^ hash.wrapping_mul(HASH_MIX))
}

/// Generates the next seed in the deterministic stream.
///
/// Both `next_seed` and `splitmix64` intentionally add `SEED_STREAM_STEP` to
/// create a non-overlapping, deterministic seed sequence compatible with the
/// seed-streaming approach. This double-increment is deliberate, not a bug.
fn next_seed(state: &mut u64) -> Seed {
    *state = state.wrapping_add(SEED_STREAM_STEP);
    Seed(splitmix64(*state))
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
