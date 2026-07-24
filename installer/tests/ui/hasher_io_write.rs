//! Compile-fail guard: `sha2` 0.11 `Sha256` no longer implements `io::Write`.
//!
//! Before 0.11, `io::copy(&mut reader, &mut hasher)` compiled because `Sha256`
//! implemented `std::io::Write`. In 0.11 that impl is gone, so the installer
//! feeds the hasher with an explicit buffered read loop instead. This fixture
//! pins the removal.

use sha2::{Digest, Sha256};
use std::io;

fn main() {
    let mut hasher = Sha256::new();
    let data: &[u8] = b"abc";
    let mut reader = data;
    io::copy(&mut reader, &mut hasher).unwrap();
    let _ = hasher.finalize();
}
