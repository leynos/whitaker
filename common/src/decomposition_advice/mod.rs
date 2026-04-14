//! Shared decomposition analysis for brain-trust diagnostics.
//!
//! This module turns pre-extracted per-method metadata into feature vectors,
//! builds a similarity graph, detects method communities, and returns
//! structured decomposition suggestions. The implementation is compiler
//! independent and accepts plain Rust values so lint drivers can populate it
//! from High-level Intermediate Representation (HIR) traversal without adding
//! `rustc_private` dependencies to `common`.

pub(crate) mod community;
mod note;
mod profile;
mod suggestion;
mod vector;

#[cfg(test)]
mod tests;

pub use note::format_diagnostic_note;
pub use profile::{DecompositionContext, MethodProfile, MethodProfileBuilder, SubjectKind};
pub use suggestion::{DecompositionSuggestion, SuggestedExtractionKind, suggest_decomposition};
pub(crate) use vector::{
    build_feature_vector, dot_product, methods_meet_cosine_threshold, minimal_feature_vector,
};
