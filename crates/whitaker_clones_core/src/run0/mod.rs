//! Token-pass acceptance and SARIF Run 0 emission.

mod emit;
mod error;
mod score;
mod span;
mod types;

pub use emit::{accept_candidate_pairs, emit_run0};
pub use error::{Run0Error, Run0Result};
pub use score::{SimilarityRatio, SimilarityThreshold};
pub use types::{AcceptedPair, TokenFragment, TokenPassConfig};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_emit;
