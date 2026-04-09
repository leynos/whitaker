//! Kani harnesses for bounded `LshConfig::new` verification.
//!
//! Run directly with:
//!
//! ```bash
//! cargo kani --manifest-path crates/whitaker_clones_core/Cargo.toml \
//!   --harness verify_lsh_config_new_symbolic
//! ```
//!
//! Or through the repository wrapper:
//!
//! ```bash
//! make kani-clone-detector
//! ```

use super::{IndexError, LshConfig, MINHASH_SIZE};

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
