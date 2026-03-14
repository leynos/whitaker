//! Threshold evaluation and escalation rules for brain trait detection.
//!
//! Provides a pure evaluation function that classifies a trait as pass,
//! warn, or deny based on configurable thresholds applied to the two
//! brain trait signals (total method count, default method cognitive
//! complexity (CC) sum). Diagnostic formatting lives in the sibling
//! `diagnostic` module and is re-exported here for convenience.
//!
//! The warn rule is AND-based: both warn conditions must hold
//! simultaneously. The deny rule is OR-based: exceeding the method
//! count deny threshold triggers denial regardless of CC. Deny
//! supersedes warn.
//!
//! See `docs/brain-trust-lints-design.md` §`brain_trait` rule set for
//! the full design rationale.

use super::TraitMetrics;

pub use super::diagnostic::{
    BrainTraitDiagnostic, format_decomposition_note, format_help, format_note,
    format_primary_message,
};

#[cfg(test)]
#[path = "evaluation_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Disposition
// ---------------------------------------------------------------------------

/// Outcome of evaluating brain trait thresholds against measured metrics.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::BrainTraitDisposition;
///
/// let d = BrainTraitDisposition::Pass;
/// assert_ne!(d, BrainTraitDisposition::Warn);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrainTraitDisposition {
    /// All metrics are within acceptable limits.
    Pass,
    /// The warn rule fired: all warn conditions hold simultaneously.
    Warn,
    /// The deny rule fired: the method count deny threshold is exceeded.
    Deny,
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Threshold configuration for brain trait evaluation.
///
/// The warn rule fires when ALL warn conditions hold simultaneously
/// (AND-based): total method count >= `methods_warn` AND default method
/// CC sum >= `default_cc_warn`. The deny rule fires when total method
/// count >= `methods_deny` (OR-based, independent of CC). Deny
/// supersedes warn.
///
/// Construct via [`BrainTraitThresholdsBuilder`].
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::BrainTraitThresholdsBuilder;
///
/// let thresholds = BrainTraitThresholdsBuilder::new()
///     .methods_warn(25)
///     .build();
/// assert_eq!(thresholds.methods_warn(), 25);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrainTraitThresholds {
    methods_warn: usize,
    methods_deny: usize,
    default_cc_warn: usize,
}

impl BrainTraitThresholds {
    /// Total method count at or above which the warn rule's method
    /// condition is met.
    #[must_use]
    pub fn methods_warn(&self) -> usize {
        self.methods_warn
    }

    /// Total method count at or above which the deny rule triggers.
    #[must_use]
    pub fn methods_deny(&self) -> usize {
        self.methods_deny
    }

    /// Default method CC sum at or above which the warn rule's
    /// complexity condition is met.
    #[must_use]
    pub fn default_cc_warn(&self) -> usize {
        self.default_cc_warn
    }
}

// ---------------------------------------------------------------------------
// Thresholds builder
// ---------------------------------------------------------------------------

const DEFAULT_METHODS_WARN: usize = 20;
const DEFAULT_METHODS_DENY: usize = 30;
const DEFAULT_CC_WARN: usize = 40;

/// Builder for [`BrainTraitThresholds`].
///
/// All fields default to the values specified in
/// `docs/brain-trust-lints-design.md` §`brain_trait` rule set.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::BrainTraitThresholdsBuilder;
///
/// let thresholds = BrainTraitThresholdsBuilder::new()
///     .methods_warn(25)
///     .default_cc_warn(50)
///     .build();
/// assert_eq!(thresholds.methods_warn(), 25);
/// assert_eq!(thresholds.default_cc_warn(), 50);
/// assert_eq!(thresholds.methods_deny(), 30); // unchanged default
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BrainTraitThresholdsBuilder {
    methods_warn: usize,
    methods_deny: usize,
    default_cc_warn: usize,
}

impl BrainTraitThresholdsBuilder {
    /// Creates a builder with all thresholds set to their defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            methods_warn: DEFAULT_METHODS_WARN,
            methods_deny: DEFAULT_METHODS_DENY,
            default_cc_warn: DEFAULT_CC_WARN,
        }
    }

    /// Sets the method count warn threshold.
    #[must_use]
    pub fn methods_warn(mut self, value: usize) -> Self {
        self.methods_warn = value;
        self
    }

    /// Sets the method count deny threshold.
    #[must_use]
    pub fn methods_deny(mut self, value: usize) -> Self {
        self.methods_deny = value;
        self
    }

    /// Sets the default method CC sum warn threshold.
    #[must_use]
    pub fn default_cc_warn(mut self, value: usize) -> Self {
        self.default_cc_warn = value;
        self
    }

    /// Consumes the builder and returns the completed thresholds.
    #[must_use]
    pub fn build(self) -> BrainTraitThresholds {
        BrainTraitThresholds {
            methods_warn: self.methods_warn,
            methods_deny: self.methods_deny,
            default_cc_warn: self.default_cc_warn,
        }
    }
}

impl Default for BrainTraitThresholdsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Computes total method count (required + default), excluding
/// associated types and consts.
fn total_method_count(metrics: &TraitMetrics) -> usize {
    metrics.required_method_count() + metrics.default_method_count()
}

/// Returns `true` when the deny condition holds (OR-based).
#[must_use]
fn is_deny_triggered(metrics: &TraitMetrics, thresholds: &BrainTraitThresholds) -> bool {
    total_method_count(metrics) >= thresholds.methods_deny
}

/// Returns `true` when all warn conditions hold simultaneously (AND-based).
#[must_use]
fn is_warn_triggered(metrics: &TraitMetrics, thresholds: &BrainTraitThresholds) -> bool {
    total_method_count(metrics) >= thresholds.methods_warn
        && metrics.default_method_cc_sum() >= thresholds.default_cc_warn
}

/// Evaluates brain trait thresholds against the given trait metrics.
///
/// Returns [`BrainTraitDisposition::Deny`] when the method count deny
/// threshold is reached (OR-based). Returns
/// [`BrainTraitDisposition::Warn`] when both the method count warn
/// threshold and the default method CC warn threshold are reached
/// simultaneously (AND-based). Returns
/// [`BrainTraitDisposition::Pass`] otherwise. Deny supersedes warn.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitThresholdsBuilder, evaluate_brain_trait,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let thresholds = BrainTraitThresholdsBuilder::new().build();
/// let metrics = TraitMetricsBuilder::new("Safe").build();
/// let disposition = evaluate_brain_trait(&metrics, &thresholds);
/// assert_eq!(
///     disposition,
///     common::brain_trait_metrics::evaluation::BrainTraitDisposition::Pass,
/// );
/// ```
#[must_use]
pub fn evaluate_brain_trait(
    metrics: &TraitMetrics,
    thresholds: &BrainTraitThresholds,
) -> BrainTraitDisposition {
    // Deny is OR-based: method count alone triggers deny.
    if is_deny_triggered(metrics, thresholds) {
        return BrainTraitDisposition::Deny;
    }

    // Warn is AND-based: both conditions must hold.
    if is_warn_triggered(metrics, thresholds) {
        return BrainTraitDisposition::Warn;
    }

    BrainTraitDisposition::Pass
}
