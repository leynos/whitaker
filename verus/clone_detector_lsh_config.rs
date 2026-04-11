//! Verus proof for the `LshConfig::new` constructor contract.
//!
//! This module mirrors the runtime branch structure in `LshConfig::new` and
//! `validate_product`: reject zero bands first, reject zero rows second, and
//! then accept only non-zero inputs whose `checked_mul` product is exactly the
//! fixed MinHash sketch width of 128.
//!
//! The sidecar proof still does not verify the compiled Rust body directly.
//! In the current repository setup, linking this file to the production
//! function would require trusted Verus assumptions for external code. The
//! proof therefore stays honest about its scope: it proves a faithful model of
//! the constructor logic, while Kani executes the real implementation.

use vstd::prelude::*;

verus! {

enum LshConfigOutcome {
    Ok(nat, nat),
    ZeroBands,
    ZeroRows,
    InvalidBandRowProduct(nat, nat),
}

spec fn minhash_size() -> nat {
    128nat
}

spec fn usize_max() -> nat {
    usize::MAX as nat
}

spec fn checked_product(bands: nat, rows: nat) -> Option<nat> {
    let product = bands * rows;
    if product <= usize_max() {
        Option::Some(product)
    } else {
        Option::None
    }
}

spec fn validate_product_accepts(bands: nat, rows: nat) -> bool {
    match checked_product(bands, rows) {
        Option::Some(product) => product == minhash_size(),
        Option::None => false,
    }
}

spec fn lsh_config_new_result(bands: nat, rows: nat) -> LshConfigOutcome {
    if bands == 0 {
        LshConfigOutcome::ZeroBands
    } else if rows == 0 {
        LshConfigOutcome::ZeroRows
    } else if validate_product_accepts(bands, rows) {
        LshConfigOutcome::Ok(bands, rows)
    } else {
        LshConfigOutcome::InvalidBandRowProduct(bands, rows)
    }
}

spec fn constructor_accepts(bands: nat, rows: nat) -> bool {
    match lsh_config_new_result(bands, rows) {
        LshConfigOutcome::Ok(_, _) => true,
        _ => false,
    }
}

proof fn lemma_minhash_size_fits_usize()
    ensures
        minhash_size() <= usize_max(),
{
    assert(minhash_size() == 128nat) by (compute);
}

proof fn lemma_zero_bands_rejected(rows: nat)
    ensures
        lsh_config_new_result(0, rows) == LshConfigOutcome::ZeroBands,
        !constructor_accepts(0, rows),
{
}

proof fn lemma_zero_rows_rejected(bands: nat)
    requires
        bands > 0,
    ensures
        lsh_config_new_result(bands, 0) == LshConfigOutcome::ZeroRows,
        !constructor_accepts(bands, 0),
{
}

#[verifier::nonlinear]
proof fn lemma_exact_product_is_accepted(bands: nat, rows: nat)
    requires
        bands > 0,
        rows > 0,
        bands * rows == minhash_size(),
    ensures
        checked_product(bands, rows) == Option::Some(minhash_size()),
        validate_product_accepts(bands, rows),
        lsh_config_new_result(bands, rows) == LshConfigOutcome::Ok(bands, rows),
        constructor_accepts(bands, rows),
{
    lemma_minhash_size_fits_usize();
    assert(bands * rows <= usize_max());
    assert(checked_product(bands, rows) == Option::Some(bands * rows));
    assert(bands * rows == minhash_size());
}

#[verifier::nonlinear]
proof fn lemma_invalid_non_zero_product_is_rejected(bands: nat, rows: nat)
    requires
        bands > 0,
        rows > 0,
        bands * rows <= usize_max(),
        bands * rows != minhash_size(),
    ensures
        checked_product(bands, rows) == Option::Some(bands * rows),
        !validate_product_accepts(bands, rows),
        lsh_config_new_result(bands, rows)
            == LshConfigOutcome::InvalidBandRowProduct(bands, rows),
        !constructor_accepts(bands, rows),
{
    assert(checked_product(bands, rows) == Option::Some(bands * rows));
}

#[verifier::nonlinear]
proof fn lemma_overflow_product_is_rejected(bands: nat, rows: nat)
    requires
        bands > 0,
        rows > 0,
        bands * rows > usize_max(),
    ensures
        checked_product(bands, rows) == Option::<nat>::None,
        !validate_product_accepts(bands, rows),
        lsh_config_new_result(bands, rows)
            == LshConfigOutcome::InvalidBandRowProduct(bands, rows),
        !constructor_accepts(bands, rows),
{
    assert(checked_product(bands, rows) == Option::<nat>::None);
}

#[verifier::nonlinear]
proof fn lemma_acceptance_matches_runtime_contract(bands: nat, rows: nat)
    ensures
        constructor_accepts(bands, rows)
            <==> (bands > 0 && rows > 0 && bands * rows == minhash_size()),
{
    if bands == 0 {
        lemma_zero_bands_rejected(rows);
    } else if rows == 0 {
        lemma_zero_rows_rejected(bands);
    } else if bands * rows == minhash_size() {
        lemma_exact_product_is_accepted(bands, rows);
    } else if bands * rows <= usize_max() {
        lemma_invalid_non_zero_product_is_rejected(bands, rows);
    } else {
        lemma_overflow_product_is_rejected(bands, rows);
    }
}

proof fn lemma_documented_examples()
    ensures
        lsh_config_new_result(32, 4) == LshConfigOutcome::Ok(32, 4),
        lsh_config_new_result(1, 128) == LshConfigOutcome::Ok(1, 128),
        lsh_config_new_result(16, 16)
            == LshConfigOutcome::InvalidBandRowProduct(16, 16),
        lsh_config_new_result(3, 42)
            == LshConfigOutcome::InvalidBandRowProduct(3, 42),
        lsh_config_new_result(usize::MAX as nat, 2)
            == LshConfigOutcome::InvalidBandRowProduct(usize::MAX as nat, 2),
{
    lemma_exact_product_is_accepted(32, 4);
    lemma_exact_product_is_accepted(1, 128);
    lemma_invalid_non_zero_product_is_rejected(16, 16);
    lemma_invalid_non_zero_product_is_rejected(3, 42);
    lemma_overflow_product_is_rejected(usize::MAX as nat, 2);
}

fn main() {
}

} // verus!
