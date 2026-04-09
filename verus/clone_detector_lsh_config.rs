//! Verus proof for the `LshConfig::new` constructor contract.
//!
//! This module models the clone-detector LSH configuration boundary
//! semantically: both dimensions must be positive and their product must equal
//! the fixed MinHash sketch width of 128.

use vstd::prelude::*;

verus! {

spec fn minhash_size() -> int {
    128
}

spec fn lsh_config_accepts(bands: int, rows: int) -> bool {
    bands > 0 && rows > 0 && bands * rows == minhash_size()
}

proof fn lemma_zero_bands_rejected(rows: int)
    ensures
        !lsh_config_accepts(0, rows),
{
}

proof fn lemma_zero_rows_rejected(bands: int)
    ensures
        !lsh_config_accepts(bands, 0),
{
}

proof fn lemma_exact_product_is_accepted()
    ensures
        lsh_config_accepts(32, 4),
        lsh_config_accepts(1, 128),
{
    assert(lsh_config_accepts(32, 4)) by (compute);
    assert(lsh_config_accepts(1, 128)) by (compute);
}

proof fn lemma_invalid_non_zero_product_is_rejected()
    ensures
        !lsh_config_accepts(16, 16),
        !lsh_config_accepts(3, 42),
{
    assert(!lsh_config_accepts(16, 16)) by (compute);
    assert(!lsh_config_accepts(3, 42)) by (compute);
}

proof fn lemma_acceptance_matches_documented_contract(bands: int, rows: int)
    ensures
        lsh_config_accepts(bands, rows)
            <==> (bands > 0 && rows > 0 && bands * rows == minhash_size()),
{
}

fn main() {
}

} // verus!
