//! Token building blocks for Whitaker clone detection.
//!
//! This crate hosts the pure-library algorithms that underpin Whitaker's clone
//! detector. Roadmap items 7.2.1 and 7.2.2 introduce:
//!
//! - `rustc_lexer`-based normalization for Type-1 and Type-2 clone profiles.
//! - `k`-shingling over normalized token streams.
//! - 64-bit Rabin-Karp rolling hashes for shingles.
//! - Winnowing to retain stable representative fingerprints.
//! - Deterministic `MinHash` sketches over retained fingerprints.
//! - Locality-sensitive hashing (LSH) candidate generation.
//! - Token-pass acceptance and SARIF Run 0 emission for accepted pairs.

pub mod ast;
mod clone_rule;
mod hashing;
pub mod index;
mod pair_result;
pub mod run0;
pub mod run1;
#[cfg(test)]
mod test_support;
pub mod token;

pub use ast::{
    AstError,
    AstHash,
    AstResult,
    ByteSpan,
    KindCounts,
    KindHistogram,
    KindWeight,
    NormalizedTree,
    Production,
    ProductionMultiset,
    canonical_hash,
    kind_counts,
    kind_histogram,
    lower_span,
    production_multiset,
    weighted_histogram,
};
pub use clone_rule::CloneRule;
pub use index::{
    CandidatePair,
    FragmentId,
    IndexError,
    IndexResult,
    LshConfig,
    LshIndex,
    MINHASH_SIZE,
    MinHashSignature,
    MinHasher,
};
pub use run0::{
    AcceptedPair,
    FragmentLocation,
    Run0Error,
    Run0Result,
    SimilarityRatio,
    SimilarityThreshold,
    TokenFragment,
    TokenPassConfig,
    accept_candidate_pairs,
    emit_run0,
};
pub use run1::{
    AstEvidence,
    AstPassConfig,
    CosineMicros,
    RefinedPair,
    RefinementInput,
    Run1Error,
    Run1Result,
    ThresholdKind,
    emit_run1,
    refine_candidate_pairs,
    validate_decimal_threshold,
    validate_threshold_order,
};
pub use token::{
    Fingerprint,
    IdentifierSymbol,
    LiteralSymbol,
    NormProfile,
    NormalizedToken,
    NormalizedTokenKind,
    Result,
    ShingleSize,
    TokenPassError,
    WinnowWindow,
    hash_shingles,
    normalize,
    winnow,
};
// Re-exported because `RefinedPair::baseline_state` and its SARIF properties
// are part of this crate's public surface.
pub use whitaker_sarif::baseline_state::BaselineState;
