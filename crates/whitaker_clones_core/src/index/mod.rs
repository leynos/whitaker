//! MinHash and LSH indexing for token-pass candidate generation.

mod error;
#[cfg(kani)]
mod kani;
mod lsh;
mod minhash;
mod types;

pub use error::{IndexError, IndexResult};
pub use lsh::LshIndex;
pub use minhash::MinHasher;
pub use types::{CandidatePair, FragmentId, LshConfig, MINHASH_SIZE, MinHashSignature};

#[cfg(test)]
mod tests;
