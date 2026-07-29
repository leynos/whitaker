//! Compile-fail guard: `sha2` 0.11 digests no longer implement `LowerHex`.
//!
//! Before 0.11, `format!("{:x}", Sha256::digest(..))` compiled because the
//! digest was a `GenericArray` implementing `core::fmt::LowerHex`. In 0.11 it
//! is a `hybrid_array::Array<u8, _>`, which does not, so this must fail. The
//! installer renders digests through `whitaker_installer`'s `to_lower_hex`
//! helper instead.

use sha2::{Digest, Sha256};

fn main() {
    let digest = Sha256::digest(b"abc");
    let _ = format!("{digest:x}");
}
