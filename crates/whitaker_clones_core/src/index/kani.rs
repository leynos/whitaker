//! Kani harnesses for bounded clone-detector index verification.
//!
//! Run directly with:
//!
//! ```bash
//! cargo kani --manifest-path crates/whitaker_clones_core/Cargo.toml \
//!   --harness verify_min_hasher_lane_minimum_for_boundary_hashes
//! ```
//!
//! Or through the repository wrapper:
//!
//! ```bash
//! make kani-clone-detector
//! ```
//!
//! The harness set deliberately splits bounded semantic coverage from overflow
//! coverage:
//!
//! - `verify_lsh_config_new_smoke` checks one accepted concrete path.
//! - `verify_lsh_config_new_symbolic` exhausts the constructor across the
//!   bounded `[0, 128]²` input space.
//! - `verify_lsh_config_new_overflow_product` drives the `checked_mul(None)`
//!   branch with non-zero overflowing inputs.
//! - `verify_min_hasher_sketch_rejects_empty_input` checks the empty
//!   retained-fingerprint boundary.
//! - `verify_min_hasher_lane_minimum_for_boundary_hashes` checks the
//!   production lane minimum for fixed full-width boundary hashes.
//! - `verify_min_hasher_unique_hashes_sorts_and_deduplicates` checks the
//!   production normalization of a fixed, unsorted input with duplicates.
//! - `verify_lsh_index_rejects_self_pairs` checks that repeated insertion of
//!   one fragment cannot produce a self-pair.
//! - `verify_lsh_index_canonicalizes_pair_order` checks that reverse lexical
//!   insertion still emits one canonical pair.
//! - `verify_lsh_index_deduplicates_repeated_band_collisions` checks that two
//!   fragments colliding in two bands still emit one candidate pair.
//! - `verify_lsh_index_is_insertion_order_independent` checks that a bounded
//!   three-fragment index produces the same candidates in forward and reverse
//!   insertion order.
//!
//! The MinHash proofs stop at one production lane and the input-normalization
//! helper; they do not construct or compare complete 128-lane signatures.
//! Runtime tests cover full-signature determinism, duplicate semantics, and
//! identical-set order independence.

use crate::token::Fingerprint;

use super::minhash::{minimum_mixed_hash_for_kani, mix_hash_for_kani, unique_hashes};
use super::{
    CandidatePair, FragmentId, IndexError, LshConfig, LshIndex, MINHASH_SIZE, MinHashSignature,
    MinHasher,
};

const KANI_MINHASH_SEED: u64 = 0xA076_1D64_78BD_642F;
const KANI_LSH_UNWIND: usize = 7;
const _: () = assert!(KANI_LSH_UNWIND == super::lsh::KANI_MAX_RECORDED_PAIRS + 1);

fn fingerprint(hash: u64, start: usize) -> Fingerprint {
    Fingerprint::new(hash, start..start.saturating_add(1))
}

fn fragment(id: &str) -> FragmentId {
    FragmentId::from(id)
}

fn repeated_signature(value: u64) -> MinHashSignature {
    MinHashSignature::new(&[value; MINHASH_SIZE])
}

fn two_band_config() -> LshConfig {
    LshConfig::new(2, MINHASH_SIZE / 2).expect("two-band config should validate")
}

fn single_band_config() -> LshConfig {
    LshConfig::new(1, MINHASH_SIZE).expect("single-band config should validate")
}

fn expected_pair(left: &str, right: &str) -> CandidatePair {
    CandidatePair::new(fragment(left), fragment(right)).expect("distinct fragments should pair")
}

fn assert_lsh_summary_matches_single_pair(index: &LshIndex, expected: CandidatePair) {
    let summary = index.candidate_pair_summary_for_kani();
    kani::assert(
        summary.unique_pair_count == 1,
        "bounded LSH scenario must emit exactly one unique candidate pair",
    );
    kani::assert(
        summary.first_pair == Some(expected),
        "bounded LSH scenario must keep the expected canonical pair",
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_lsh_config_new_smoke() {
    let config = match LshConfig::new(32, 4) {
        Ok(config) => config,
        Err(error) => panic!("expected valid LSH config, got {error:?}"),
    };

    kani::assert(config.bands() == 32, "smoke harness should keep band count");
    kani::assert(config.rows() == 4, "smoke harness should keep row count");
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_lsh_config_new_symbolic() {
    let bands: usize = kani::any();
    let rows: usize = kani::any();
    kani::assume(bands <= MINHASH_SIZE);
    kani::assume(rows <= MINHASH_SIZE);

    match LshConfig::new(bands, rows) {
        Ok(config) => {
            kani::assert(bands > 0, "accepted configs must reject zero bands");
            kani::assert(rows > 0, "accepted configs must reject zero rows");
            kani::assert(
                bands * rows == MINHASH_SIZE,
                "accepted configs must match the fixed sketch width",
            );
            kani::assert(config.bands() == bands, "accepted config keeps band count");
            kani::assert(config.rows() == rows, "accepted config keeps row count");
        }
        Err(IndexError::ZeroBands) => {
            kani::assert(bands == 0, "ZeroBands must mean the input bands were zero");
        }
        Err(IndexError::ZeroRows) => {
            kani::assert(
                bands != 0,
                "ZeroRows occurs only after non-zero bands validate",
            );
            kani::assert(rows == 0, "ZeroRows must mean the input rows were zero");
        }
        Err(IndexError::InvalidBandRowProduct {
            bands: actual_bands,
            rows: actual_rows,
            expected,
        }) => {
            kani::assert(actual_bands == bands, "error should report the input bands");
            kani::assert(actual_rows == rows, "error should report the input rows");
            kani::assert(
                expected == MINHASH_SIZE,
                "error should report the fixed MinHash size",
            );
            kani::assert(
                bands > 0,
                "invalid product errors are only possible after zero-band validation",
            );
            kani::assert(
                rows > 0,
                "invalid product errors are only possible after zero-row validation",
            );
            kani::assert(
                bands.checked_mul(rows) != Some(MINHASH_SIZE),
                "invalid product errors require a non-matching product",
            );
        }
        Err(IndexError::EmptyFingerprintSet) => {
            kani::assert(false, "LshConfig::new must not produce fingerprint errors");
        }
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_lsh_config_new_overflow_product() {
    let bands: usize = kani::any();
    let rows = 2usize;
    kani::assume(bands > 0);
    kani::assume(bands > usize::MAX / rows);
    kani::assert(
        bands.checked_mul(rows).is_none(),
        "overflow harness must drive the checked_mul(None) branch",
    );

    match LshConfig::new(bands, rows) {
        Ok(_) => {
            kani::assert(false, "overflowing products must be rejected");
        }
        Err(IndexError::ZeroBands) => {
            kani::assert(false, "overflow harness assumes non-zero bands");
        }
        Err(IndexError::ZeroRows) => {
            kani::assert(false, "overflow harness assumes non-zero rows");
        }
        Err(IndexError::InvalidBandRowProduct {
            bands: actual_bands,
            rows: actual_rows,
            expected,
        }) => {
            kani::assert(actual_bands == bands, "error should report the input bands");
            kani::assert(actual_rows == rows, "error should report the input rows");
            kani::assert(
                expected == MINHASH_SIZE,
                "error should report the fixed MinHash size",
            );
            kani::assert(
                bands.checked_mul(rows).is_none(),
                "overflow harness must keep the overflowing product precondition",
            );
        }
        Err(IndexError::EmptyFingerprintSet) => {
            kani::assert(false, "LshConfig::new must not produce fingerprint errors");
        }
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_min_hasher_sketch_rejects_empty_input() {
    let hasher = MinHasher::from_seed_for_kani(KANI_MINHASH_SEED);

    match hasher.sketch(&[]) {
        Err(IndexError::EmptyFingerprintSet) => {}
        Ok(_) => kani::assert(false, "empty input must not produce a signature"),
        Err(_) => kani::assert(false, "empty input must report EmptyFingerprintSet"),
    }
}

/// Verify the production lane minimum for distinct full-width boundary hashes.
#[kani::proof]
#[kani::unwind(3)]
fn verify_min_hasher_lane_minimum_for_boundary_hashes() {
    let hashes = [0, u64::MAX];
    let left = mix_hash_for_kani(KANI_MINHASH_SEED, hashes[0]);
    let right = mix_hash_for_kani(KANI_MINHASH_SEED, hashes[1]);
    let actual = minimum_mixed_hash_for_kani(KANI_MINHASH_SEED, &hashes);

    kani::assert(
        left != right,
        "boundary hashes must exercise a distinct mixed-value comparison",
    );
    kani::assert(
        actual == left.min(right),
        "a MinHash lane must equal the minimum mixed value for each input hash",
    );
}

/// Verify production normalization sorts and deduplicates fixed fingerprints.
#[kani::proof]
#[kani::unwind(8)]
fn verify_min_hasher_unique_hashes_sorts_and_deduplicates() {
    let fingerprints = [
        fingerprint(u64::MAX, 0),
        fingerprint(17, 1),
        fingerprint(17, 2),
        fingerprint(0, 3),
    ];

    match unique_hashes(&fingerprints) {
        Ok(hashes) => {
            kani::assert(
                hashes.len() == 3,
                "normalization must remove duplicate hashes",
            );
            kani::assert(
                hashes[0] == 0,
                "normalization must sort the smallest hash first",
            );
            kani::assert(hashes[1] == 17, "normalization must retain one middle hash");
            kani::assert(
                hashes[2] == u64::MAX,
                "normalization must sort the largest hash last",
            );
        }
        Err(_) => kani::assert(false, "non-empty fingerprints must normalize"),
    }
}

#[kani::proof]
#[kani::unwind(7)]
fn verify_lsh_index_rejects_self_pairs() {
    let signature = repeated_signature(11);
    let alpha = fragment("a");
    let mut index = LshIndex::new(single_band_config());

    index.insert(&alpha, &signature);
    index.insert(&alpha, &signature);

    let summary = index.candidate_pair_summary_for_kani();
    kani::assert(
        summary.unique_pair_count == 0,
        "repeated insertion of one fragment must not emit self-pairs",
    );
    kani::assert(
        summary.first_pair.is_none(),
        "self-pair rejection must not retain a candidate pair",
    );
}

#[kani::proof]
#[kani::unwind(7)]
fn verify_lsh_index_canonicalizes_pair_order() {
    let signature = repeated_signature(13);
    let alpha = fragment("a");
    let beta = fragment("b");
    let mut index = LshIndex::new(single_band_config());

    index.insert(&beta, &signature);
    index.insert(&alpha, &signature);

    assert_lsh_summary_matches_single_pair(&index, expected_pair("a", "b"));
}

#[kani::proof]
#[kani::unwind(7)]
fn verify_lsh_index_deduplicates_repeated_band_collisions() {
    let signature = repeated_signature(17);
    let alpha = fragment("a");
    let beta = fragment("b");
    let mut index = LshIndex::new(two_band_config());

    index.insert(&alpha, &signature);
    index.insert(&beta, &signature);

    assert_lsh_summary_matches_single_pair(&index, expected_pair("a", "b"));
}

#[kani::proof]
#[kani::unwind(7)]
fn verify_lsh_index_is_insertion_order_independent() {
    let shared = repeated_signature(19);
    let distinct = repeated_signature(23);
    let alpha = fragment("a");
    let beta = fragment("b");
    let gamma = fragment("c");

    let mut forward = LshIndex::new(single_band_config());
    forward.insert(&alpha, &shared);
    forward.insert(&beta, &shared);
    forward.insert(&gamma, &distinct);

    let mut reverse = LshIndex::new(single_band_config());
    reverse.insert(&gamma, &distinct);
    reverse.insert(&beta, &shared);
    reverse.insert(&alpha, &shared);

    let forward_candidates = forward.candidate_pair_summary_for_kani();
    let reverse_candidates = reverse.candidate_pair_summary_for_kani();
    kani::assert(
        forward_candidates == reverse_candidates,
        "candidate generation must be independent of fragment insertion order",
    );
    kani::assert(
        forward_candidates.unique_pair_count == 1,
        "bounded insertion-order scenario must emit one shared candidate",
    );
    kani::assert(
        forward_candidates.first_pair == Some(expected_pair("a", "b")),
        "insertion-order scenario must keep the expected canonical pair",
    );
}
