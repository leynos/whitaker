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
//! - `verify_min_hasher_sketch_is_deterministic` exhausts symbolic hash values
//!   for a fixed three-fingerprint input.
//! - `verify_min_hasher_sketch_ignores_duplicate_hashes` compares a symbolic
//!   input against the same hash set with repeated hashes at different ranges.

use crate::token::Fingerprint;

use super::{IndexError, LshConfig, MINHASH_SIZE, MinHasher};

fn fingerprint(hash: u64, start: usize) -> Fingerprint {
    Fingerprint::new(hash, start..start.saturating_add(1))
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
    let hasher = MinHasher::new();

    match hasher.sketch(&[]) {
        Err(IndexError::EmptyFingerprintSet) => {}
        Ok(_) => kani::assert(false, "empty input must not produce a signature"),
        Err(_) => kani::assert(false, "empty input must report EmptyFingerprintSet"),
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_min_hasher_sketch_is_deterministic() {
    let hashes = [kani::any::<u64>(), kani::any::<u64>(), kani::any::<u64>()];
    let fingerprints = [
        fingerprint(hashes[0], 0),
        fingerprint(hashes[1], 1),
        fingerprint(hashes[2], 2),
    ];

    let left = MinHasher::new()
        .sketch(&fingerprints)
        .expect("non-empty fingerprints should sketch");
    let right = MinHasher::new()
        .sketch(&fingerprints)
        .expect("non-empty fingerprints should sketch");

    kani::assert(
        left.values() == right.values(),
        "sketching the same fingerprints must be deterministic",
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn verify_min_hasher_sketch_ignores_duplicate_hashes() {
    let left_hash = kani::any::<u64>();
    let right_hash = kani::any::<u64>();
    let unique = [fingerprint(left_hash, 0), fingerprint(right_hash, 1)];
    let duplicated = [
        fingerprint(left_hash, 0),
        fingerprint(right_hash, 1),
        fingerprint(left_hash, 2),
        fingerprint(right_hash, 3),
    ];

    let hasher = MinHasher::new();
    let unique_signature = hasher
        .sketch(&unique)
        .expect("non-empty fingerprints should sketch");
    let duplicated_signature = hasher
        .sketch(&duplicated)
        .expect("non-empty fingerprints should sketch");

    kani::assert(
        unique_signature.values() == duplicated_signature.values(),
        "duplicate fingerprint hashes must not change the sketch",
    );
}
