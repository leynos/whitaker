//! Kani harnesses for bounded clone-detector index verification.
//!
//! Run directly with:
//!
//! ```bash
//! cargo kani --manifest-path crates/whitaker_clones_core/Cargo.toml \
//!   --harness verify_min_hasher_sketch_is_deterministic
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
//! - `verify_min_hasher_sketch_is_deterministic` checks first, middle, and last
//!   signature lanes for wide boundary-hash inputs.
//! - `verify_min_hasher_sketch_ignores_duplicate_hashes` compares a wide
//!   boundary-hash set against the same set with repeated hashes.
//! - `verify_lsh_index_rejects_self_pairs` checks that repeated insertion of
//!   one fragment cannot produce a self-pair.
//! - `verify_lsh_index_canonicalizes_pair_order` checks that reverse lexical
//!   insertion still emits one canonical pair.
//! - `verify_lsh_index_deduplicates_repeated_band_collisions` checks that two
//!   fragments colliding in two bands still emit one candidate pair.
//! - `verify_lsh_index_is_insertion_order_independent` checks that a bounded
//!   three-fragment index produces the same candidates in forward and reverse
//!   insertion order.

use crate::token::Fingerprint;

use super::{
    CandidatePair, FragmentId, IndexError, LshConfig, LshIndex, MINHASH_SIZE, MinHashSignature,
    MinHasher,
};

const KANI_MINHASH_SEED: u64 = 0xA076_1D64_78BD_642F;
const KANI_MINHASH_MIDDLE_SEED: u64 = 0xE703_7ED1_A0B4_28DB;
const KANI_MINHASH_LAST_SEED: u64 = 0x8EBC_6AF0_9C88_C6E3;

fn fingerprint(hash: u64, start: usize) -> Fingerprint {
    Fingerprint::new(hash, start..start.saturating_add(1))
}

fn checked_lane_hasher() -> MinHasher {
    MinHasher::from_checked_lane_seeds_for_kani(
        KANI_MINHASH_SEED,
        KANI_MINHASH_MIDDLE_SEED,
        KANI_MINHASH_LAST_SEED,
    )
}

fn fragment(id: &str) -> FragmentId {
    FragmentId::from(id)
}

fn repeated_signature(value: u64) -> MinHashSignature {
    MinHashSignature::new([value; MINHASH_SIZE])
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

fn assert_lane_selects_singleton_min(
    lane: usize,
    signature: &MinHashSignature,
    left_singleton: &MinHashSignature,
    right_singleton: &MinHashSignature,
) {
    let actual = signature.values()[lane];
    let left = left_singleton.values()[lane];
    let right = right_singleton.values()[lane];

    kani::assert(
        left != right,
        "singleton lane values must distinguish the checked input hashes",
    );
    kani::assert(
        actual == left || actual == right,
        "combined sketch lane must select a hash value present in a singleton sketch",
    );
    kani::assert(
        actual <= left && actual <= right,
        "combined sketch lane must contain the minimum singleton hash value",
    );
}

fn assert_checked_lanes_select_singleton_min(
    signature: &MinHashSignature,
    left_singleton: &MinHashSignature,
    right_singleton: &MinHashSignature,
) {
    kani::assert(
        signature.values().len() == MINHASH_SIZE,
        "signature must keep the fixed MinHash width",
    );
    assert_lane_selects_singleton_min(0, signature, left_singleton, right_singleton);
    assert_lane_selects_singleton_min(MINHASH_SIZE / 2, signature, left_singleton, right_singleton);
    assert_lane_selects_singleton_min(MINHASH_SIZE - 1, signature, left_singleton, right_singleton);
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

#[kani::proof]
#[kani::unwind(4)]
fn verify_min_hasher_sketch_is_deterministic() {
    let hashes = [0, u64::MAX];
    let fingerprints = [fingerprint(hashes[0], 0), fingerprint(hashes[1], 1)];

    let left = checked_lane_hasher()
        .sketch(&fingerprints)
        .expect("non-empty fingerprints should sketch");
    let right = checked_lane_hasher()
        .sketch(&fingerprints)
        .expect("non-empty fingerprints should sketch");
    let first_singleton = checked_lane_hasher()
        .sketch(&[fingerprints[0].clone()])
        .expect("single fingerprint should sketch");
    let second_singleton = checked_lane_hasher()
        .sketch(&[fingerprints[1].clone()])
        .expect("single fingerprint should sketch");

    kani::assert(
        left.values()[0] == right.values()[0],
        "sketching the same fingerprints must be deterministic for the first lane",
    );
    kani::assert(
        left.values()[MINHASH_SIZE / 2] == right.values()[MINHASH_SIZE / 2],
        "sketching the same fingerprints must be deterministic for the middle lane",
    );
    kani::assert(
        left.values()[MINHASH_SIZE - 1] == right.values()[MINHASH_SIZE - 1],
        "sketching the same fingerprints must be deterministic for the last lane",
    );
    assert_checked_lanes_select_singleton_min(&left, &first_singleton, &second_singleton);
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_min_hasher_sketch_ignores_duplicate_hashes() {
    let hashes = [u64::from(u8::MAX) + 1, u64::from(u16::MAX)];
    let unique = [fingerprint(hashes[0], 0), fingerprint(hashes[1], 1)];
    let duplicated = [
        fingerprint(hashes[0], 0),
        fingerprint(hashes[1], 1),
        fingerprint(hashes[0], 2),
        fingerprint(hashes[1], 3),
    ];

    let hasher = checked_lane_hasher();
    let unique_signature = hasher
        .sketch(&unique)
        .expect("non-empty fingerprints should sketch");
    let duplicated_signature = hasher
        .sketch(&duplicated)
        .expect("non-empty fingerprints should sketch");
    let first_singleton = hasher
        .sketch(&[unique[0].clone()])
        .expect("single fingerprint should sketch");
    let second_singleton = hasher
        .sketch(&[unique[1].clone()])
        .expect("single fingerprint should sketch");

    kani::assert(
        unique_signature.values()[0] == duplicated_signature.values()[0],
        "duplicate fingerprint hashes must not change the first signature lane",
    );
    kani::assert(
        unique_signature.values()[MINHASH_SIZE / 2]
            == duplicated_signature.values()[MINHASH_SIZE / 2],
        "duplicate fingerprint hashes must not change the middle signature lane",
    );
    kani::assert(
        unique_signature.values()[MINHASH_SIZE - 1]
            == duplicated_signature.values()[MINHASH_SIZE - 1],
        "duplicate fingerprint hashes must not change the last signature lane",
    );
    assert_checked_lanes_select_singleton_min(
        &unique_signature,
        &first_singleton,
        &second_singleton,
    );
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
