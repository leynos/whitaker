//! Token building blocks for Whitaker clone detection.
//!
//! This crate hosts the pure-library algorithms that underpin Whitaker's clone
//! detector. Roadmap item 7.2.1 introduces the token pipeline:
//!
//! - `rustc_lexer`-based normalization for Type-1 and Type-2 clone profiles.
//! - `k`-shingling over normalized token streams.
//! - 64-bit Rabin-Karp rolling hashes for shingles.
//! - Winnowing to retain stable representative fingerprints.

pub mod token;

pub use token::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedToken,
    NormalizedTokenKind, Result, ShingleSize, TokenPassError, WinnowWindow, hash_shingles,
    normalize, winnow,
};
