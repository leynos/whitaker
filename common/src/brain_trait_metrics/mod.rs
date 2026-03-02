//! Metric collection for brain trait detection.
//!
//! Provides pure data structures and helper functions for collecting the
//! three signals used by the `brain_trait` lint:
//!
//! - interface size (trait item counts),
//! - default method cognitive complexity aggregation, and
//! - implementor burden (required method count).
//!
//! These helpers are compiler-independent and accept pre-extracted metadata.
//! Lint drivers can populate this module from High-level Intermediate
//! Representation (HIR) traversal without adding `rustc_private` dependencies
//! to `common`.

mod item;
mod metrics;

#[cfg(test)]
mod tests;

pub use item::{
    TraitItemKind, TraitItemMetrics, default_method_cc_sum, default_method_count,
    required_method_count, trait_item_count,
};
pub use metrics::{TraitMetrics, TraitMetricsBuilder};
