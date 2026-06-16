//! Parser-agnostic AST feature extraction for clone refinement.
//!
//! This module is split into a pure domain and one parser adapter. The
//! dependency-rule invariant is: exactly one file under
//! `crates/whitaker_clones_core/src/ast/` may import `ra_ap_syntax`, `rowan`, or
//! `ra_ap_parser`, and that file is `ast/lowering.rs`. Domain files must never
//! use `ast::lowering`; dependency flow is adapter to domain only.

mod cover;
mod error;
mod features;
mod hash;
#[cfg(kani)]
mod kani;
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
pub use tree::{ByteSpan, Depth, KindId, LeafClass, NormalisedNode, NormalisedTree};
