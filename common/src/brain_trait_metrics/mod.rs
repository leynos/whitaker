//! Metric collection and threshold evaluation for brain trait detection.
//!
//! Provides pure data structures and helper functions for collecting the
//! three signals used by the `brain_trait` lint:
//!
//! - interface size (trait item counts),
//! - default method cognitive complexity aggregation, and
//! - implementor burden (required method count).
//!
//! Additionally provides threshold evaluation (roadmap 6.3.2) and
//! diagnostic formatting for surfacing measured values in lint output.
//!
//! These helpers are compiler-independent and accept pre-extracted metadata.
//! Lint drivers can populate this module from High-level Intermediate
//! Representation (HIR) traversal without adding `rustc_private` dependencies
//! to `common`.

pub mod diagnostic;
pub mod evaluation;
mod item;
mod metrics;

#[cfg(test)]
mod tests;

pub use evaluation::{
    BrainTraitDiagnostic, BrainTraitDisposition, BrainTraitThresholds, BrainTraitThresholdsBuilder,
    evaluate_brain_trait, format_help, format_note, format_primary_message,
};
pub use item::{
    TraitItemKind, TraitItemMetrics, default_method_cc_sum, default_method_count,
    required_method_count, trait_item_count,
};
pub use metrics::{TraitMetrics, TraitMetricsBuilder};
