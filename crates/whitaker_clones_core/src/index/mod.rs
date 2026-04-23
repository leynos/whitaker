//! MinHash and LSH indexing for token-pass candidate generation.

mod error;
mod fragment_id;
#[cfg(kani)]
mod kani;
mod lsh;
mod minhash;
mod types;

pub use error::{IndexError, IndexResult};
pub use fragment_id::FragmentId;
pub use lsh::LshIndex;
pub use minhash::MinHasher;
pub use types::{CandidatePair, LshConfig, MINHASH_SIZE, MinHashSignature};

#[cfg(test)]
mod tests;
