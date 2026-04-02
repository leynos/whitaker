//! Threshold evaluation and escalation rules for brain type detection.
//!
//! Provides a pure evaluation function that classifies a type as pass,
//! warn, or deny based on configurable thresholds applied to the four
//! brain type signals (WMC, brain method count, LCOM4, foreign reach).
//! Diagnostic formatting lives in the sibling `diagnostic` module and
//! is re-exported here for convenience.
//!
//! The warn rule is AND-based: all warn conditions must hold
//! simultaneously. The deny rule is OR-based: any single deny condition
//! triggers denial. Deny supersedes warn.
//!
//! See `docs/brain-trust-lints-design.md` §`brain_type` rule set for
//! the full design rationale.

use super::TypeMetrics;

pub use super::diagnostic::{
    BrainTypeDiagnostic, format_decomposition_note, format_help, format_note,
    format_primary_message,
};

#[cfg(test)]
#[path = "evaluation_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Disposition
// ---------------------------------------------------------------------------

/// Outcome of evaluating brain type thresholds against measured metrics.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::BrainTypeDisposition;
///
/// let d = BrainTypeDisposition::Pass;
/// assert_ne!(d, BrainTypeDisposition::Warn);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrainTypeDisposition {
    /// All metrics are within acceptable limits.
    Pass,
    /// The warn rule fired: all warn conditions hold simultaneously.
    Warn,
    /// The deny rule fired: at least one deny condition holds.
    Deny,
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Threshold configuration for brain type evaluation.
///
/// The warn rule fires when ALL warn conditions hold simultaneously
/// (AND-based). The deny rule fires when ANY single deny condition
/// holds (OR-based). Deny supersedes warn.
///
/// Construct via [`BrainTypeThresholdsBuilder`].
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::BrainTypeThresholdsBuilder;
///
/// let thresholds = BrainTypeThresholdsBuilder::new().wmc_warn(50).build();
/// assert_eq!(thresholds.wmc_warn(), 50);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrainTypeThresholds {
    wmc_warn: usize,
    wmc_deny: usize,
    lcom4_warn: usize,
    lcom4_deny: usize,
    brain_method_deny_count: usize,
}

impl BrainTypeThresholds {
    /// WMC at or above which the warn rule's WMC condition is met.
    #[must_use]
    pub fn wmc_warn(&self) -> usize {
        self.wmc_warn
    }

    /// WMC at or above which the deny rule triggers (OR-based).
    #[must_use]
    pub fn wmc_deny(&self) -> usize {
        self.wmc_deny
    }

    /// LCOM4 at or above which the warn rule's cohesion condition is met.
    #[must_use]
    pub fn lcom4_warn(&self) -> usize {
        self.lcom4_warn
    }

    /// LCOM4 at or above which the deny rule triggers (OR-based).
    #[must_use]
    pub fn lcom4_deny(&self) -> usize {
        self.lcom4_deny
    }

    /// Brain method count at or above which the deny rule triggers.
    #[must_use]
    pub fn brain_method_deny_count(&self) -> usize {
        self.brain_method_deny_count
    }
}

// ---------------------------------------------------------------------------
// Thresholds builder
// ---------------------------------------------------------------------------

const DEFAULT_WMC_WARN: usize = 60;
const DEFAULT_WMC_DENY: usize = 100;
const DEFAULT_LCOM4_WARN: usize = 2;
const DEFAULT_LCOM4_DENY: usize = 3;
const DEFAULT_BRAIN_METHOD_DENY_COUNT: usize = 2;

/// Builder for [`BrainTypeThresholds`].
///
/// All fields default to the values specified in
/// `docs/brain-trust-lints-design.md` §`brain_type` rule set.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::BrainTypeThresholdsBuilder;
///
/// let thresholds = BrainTypeThresholdsBuilder::new()
///     .wmc_warn(50)
///     .lcom4_deny(4)
///     .build();
/// assert_eq!(thresholds.wmc_warn(), 50);
/// assert_eq!(thresholds.lcom4_deny(), 4);
/// assert_eq!(thresholds.wmc_deny(), 100); // unchanged default
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BrainTypeThresholdsBuilder {
    wmc_warn: usize,
    wmc_deny: usize,
    lcom4_warn: usize,
    lcom4_deny: usize,
    brain_method_deny_count: usize,
}

impl BrainTypeThresholdsBuilder {
    /// Creates a builder with all thresholds set to their defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            wmc_warn: DEFAULT_WMC_WARN,
            wmc_deny: DEFAULT_WMC_DENY,
            lcom4_warn: DEFAULT_LCOM4_WARN,
            lcom4_deny: DEFAULT_LCOM4_DENY,
            brain_method_deny_count: DEFAULT_BRAIN_METHOD_DENY_COUNT,
        }
    }

    /// Sets the WMC warn threshold.
    #[must_use]
    pub fn wmc_warn(mut self, value: usize) -> Self {
        self.wmc_warn = value;
        self
    }

    /// Sets the WMC deny threshold.
    #[must_use]
    pub fn wmc_deny(mut self, value: usize) -> Self {
        self.wmc_deny = value;
        self
    }

    /// Sets the LCOM4 warn threshold.
    #[must_use]
    pub fn lcom4_warn(mut self, value: usize) -> Self {
        self.lcom4_warn = value;
        self
    }

    /// Sets the LCOM4 deny threshold.
    #[must_use]
    pub fn lcom4_deny(mut self, value: usize) -> Self {
        self.lcom4_deny = value;
        self
    }

    /// Sets the brain method count deny threshold.
    #[must_use]
    pub fn brain_method_deny_count(mut self, value: usize) -> Self {
        self.brain_method_deny_count = value;
        self
    }

    /// Consumes the builder and returns the completed thresholds.
    #[must_use]
    pub fn build(self) -> BrainTypeThresholds {
        BrainTypeThresholds {
            wmc_warn: self.wmc_warn,
            wmc_deny: self.wmc_deny,
            lcom4_warn: self.lcom4_warn,
            lcom4_deny: self.lcom4_deny,
            brain_method_deny_count: self.brain_method_deny_count,
        }
    }
}

impl Default for BrainTypeThresholdsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Returns `true` when any single deny condition holds (OR-based).
#[must_use]
fn is_deny_triggered(metrics: &TypeMetrics, thresholds: &BrainTypeThresholds) -> bool {
    metrics.wmc() >= thresholds.wmc_deny
        || metrics.brain_method_count() >= thresholds.brain_method_deny_count
        || metrics.lcom4() >= thresholds.lcom4_deny
}

/// Returns `true` when all warn conditions hold simultaneously (AND-based).
#[must_use]
fn is_warn_triggered(metrics: &TypeMetrics, thresholds: &BrainTypeThresholds) -> bool {
    metrics.wmc() >= thresholds.wmc_warn
        && metrics.brain_method_count() >= 1
        && metrics.lcom4() >= thresholds.lcom4_warn
}

/// Evaluates brain type thresholds against the given type metrics.
///
/// Returns [`BrainTypeDisposition::Deny`] when any single deny
/// condition holds (OR-based). Returns [`BrainTypeDisposition::Warn`]
/// when all warn conditions hold simultaneously (AND-based). Returns
/// [`BrainTypeDisposition::Pass`] otherwise. Deny supersedes warn.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeThresholdsBuilder, evaluate_brain_type,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let thresholds = BrainTypeThresholdsBuilder::new().build();
/// let metrics = TypeMetricsBuilder::new("Safe", 25, 80).build();
/// let disposition = evaluate_brain_type(&metrics, &thresholds);
/// assert_eq!(
///     disposition,
///     whitaker_common::brain_type_metrics::evaluation::BrainTypeDisposition::Pass,
/// );
/// ```
#[must_use]
pub fn evaluate_brain_type(
    metrics: &TypeMetrics,
    thresholds: &BrainTypeThresholds,
) -> BrainTypeDisposition {
    // Deny is OR-based: any single trigger fires deny.
    if is_deny_triggered(metrics, thresholds) {
        return BrainTypeDisposition::Deny;
    }

    // Warn is AND-based: all conditions must hold.
    if is_warn_triggered(metrics, thresholds) {
        return BrainTypeDisposition::Warn;
    }

    BrainTypeDisposition::Pass
}
