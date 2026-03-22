//! Token building blocks for Whitaker clone detection.
//!
//! This crate hosts the pure-library algorithms that underpin Whitaker's clone
//! detector. Roadmap items 7.2.1 and 7.2.2 introduce:
//!
//! - `rustc_lexer`-based normalization for Type-1 and Type-2 clone profiles.
//! - `k`-shingling over normalized token streams.
//! - 64-bit Rabin-Karp rolling hashes for shingles.
//! - Winnowing to retain stable representative fingerprints.
//! - Deterministic MinHash sketches over retained fingerprints.
//! - Locality-sensitive hashing (LSH) candidate generation.

pub mod index;
pub mod token;

pub use index::{
    CandidatePair, FragmentId, IndexError, LshConfig, LshIndex, MINHASH_SIZE, MinHashSignature,
    MinHasher,
};
pub use token::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedToken,
    NormalizedTokenKind, Result, ShingleSize, TokenPassError, WinnowWindow, hash_shingles,
    normalize, winnow,
};
