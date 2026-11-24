//! Token-pass acceptance and SARIF Run 0 emission.

mod emit;
mod error;
mod location;
mod score;
mod types;

pub use emit::{accept_candidate_pairs, emit_run0};
pub use error::{Run0Error, Run0Result};
pub use location::FragmentLocation;
pub(crate) use score::jaccard_similarity;
pub use score::{SimilarityRatio, SimilarityThreshold};
pub use types::{AcceptedPair, TokenFragment, TokenPassConfig};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_emit;
