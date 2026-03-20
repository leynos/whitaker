//! Token normalization, shingling, hashing, and winnowing for clone detection.

mod error;
mod fingerprint;
mod normalize;
mod types;

pub use error::{Result, TokenPassError};
pub use fingerprint::{hash_shingles, winnow};
pub use normalize::normalize;
pub use types::{
    Fingerprint, IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedToken,
    NormalizedTokenKind, ShingleSize, WinnowWindow,
};

#[cfg(test)]
mod tests;
