//! Parser-agnostic AST feature extraction for clone refinement.
//!
//! This module is split into a pure domain and one parser adapter. The
//! dependency-rule invariant is: exactly one production module may import
//! `ra_ap_syntax`, `rowan`, or `ra_ap_parser`: `ast/lowering.rs`.
//! Adapter-scoped tests such as `ast/lowering_tests.rs` are excluded from that
//! restriction. Domain files must never use `ast::lowering`; dependency flow is
//! adapter to domain only.

mod cover;
mod error;
mod features;
mod hash;
#[cfg(kani)]
mod kani;
mod metrics;
#[cfg(not(feature = "parser"))]
mod lowering {
    //! No-parser lowering stub for verification builds.

    use std::time::Instant;

    use super::{AstError, ByteSpan, NormalizedTree, metrics::record_lower_span_metrics};

    /// Parser schema seed shared with AST hashing.
    pub const PARSER_SCHEMA_VERSION: &str = crate::hashing::PARSER_SCHEMA_VERSION;

    /// Reports that parser-backed lowering is unavailable without the
    /// `parser` feature.
    pub fn lower_span(_file_text: &str, _span: ByteSpan) -> Result<NormalizedTree, AstError> {
        let started_at = Instant::now();
        let result = Err(AstError::ParserUnavailable);
        record_lower_span_metrics(&result, started_at.elapsed(), false);
        result
    }
}
#[cfg(feature = "parser")]
mod lowering;
#[cfg(test)]
mod tests;
mod tree;

pub use cover::select_smallest_covering;
pub use error::{AstError, AstResult};
pub use features::{
    KindCounts, KindHistogram, KindWeight, Production, ProductionMultiset, kind_counts,
    kind_histogram, production_multiset, weighted_histogram,
};
pub use hash::{AstHash, canonical_hash};
pub use lowering::{PARSER_SCHEMA_VERSION, lower_span};
pub use tree::{ByteSpan, Depth, KindId, LeafClass, NormalizedNode, NormalizedTree};
