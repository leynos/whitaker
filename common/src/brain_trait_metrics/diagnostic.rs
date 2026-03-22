//! Diagnostic detail and formatting for brain trait evaluation.
//!
//! Carries measured values and disposition after threshold evaluation,
//! and provides formatting functions that produce human-readable
//! diagnostic messages for the lint driver.
//!
//! See `docs/brain-trust-lints-design.md` §Diagnostic output for the
//! full format specification.

use super::TraitMetrics;
use super::evaluation::BrainTraitDisposition;
use crate::decomposition_advice::{
    DecompositionContext, DecompositionSuggestion, SubjectKind, format_diagnostic_note,
};

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Diagnostic detail
// ---------------------------------------------------------------------------

/// Carries measured values and disposition for diagnostic rendering.
///
/// The lint driver constructs this after evaluation and passes it to
/// formatting functions that produce human-readable messages.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitDiagnostic, BrainTraitDisposition,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let metrics = TraitMetricsBuilder::new("Foo").build();
/// let diag = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Pass);
/// assert_eq!(diag.trait_name(), "Foo");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainTraitDiagnostic {
    trait_name: String,
    disposition: BrainTraitDisposition,
    required_method_count: usize,
    default_method_count: usize,
    default_method_cc_sum: usize,
    total_item_count: usize,
    implementor_burden: usize,
}

impl BrainTraitDiagnostic {
    /// Creates a diagnostic from evaluated metrics and disposition.
    #[must_use]
    pub fn new(metrics: &TraitMetrics, disposition: BrainTraitDisposition) -> Self {
        Self {
            trait_name: metrics.trait_name().to_owned(),
            disposition,
            required_method_count: metrics.required_method_count(),
            default_method_count: metrics.default_method_count(),
            default_method_cc_sum: metrics.default_method_cc_sum(),
            total_item_count: metrics.total_item_count(),
            implementor_burden: metrics.implementor_burden(),
        }
    }

    /// Returns the trait name.
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the evaluation disposition.
    #[must_use]
    pub fn disposition(&self) -> BrainTraitDisposition {
        self.disposition
    }

    /// Returns the number of required methods.
    #[must_use]
    pub fn required_method_count(&self) -> usize {
        self.required_method_count
    }

    /// Returns the number of default methods.
    #[must_use]
    pub fn default_method_count(&self) -> usize {
        self.default_method_count
    }

    /// Returns the total method count (required + default).
    #[must_use]
    pub fn total_method_count(&self) -> usize {
        self.required_method_count + self.default_method_count
    }

    /// Returns the sum of default method cognitive complexity values.
    #[must_use]
    pub fn default_method_cc_sum(&self) -> usize {
        self.default_method_cc_sum
    }

    /// Returns the total number of trait items (methods + associated
    /// types + associated consts).
    #[must_use]
    pub fn total_item_count(&self) -> usize {
        self.total_item_count
    }

    /// Returns implementor burden (required method count).
    #[must_use]
    pub fn implementor_burden(&self) -> usize {
        self.implementor_burden
    }
}

// ---------------------------------------------------------------------------
// Diagnostic formatting
// ---------------------------------------------------------------------------

/// Formats the primary diagnostic message with measured values.
///
/// Includes method count breakdown (required vs default) and default
/// method CC sum when non-zero.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitDiagnostic, BrainTraitDisposition, format_primary_message,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let mut builder = TraitMetricsBuilder::new("Parser");
/// builder.add_required_method("parse");
/// builder.add_default_method("render", 12, false);
/// let metrics = builder.build();
/// let diag = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Warn);
/// let msg = format_primary_message(&diag);
/// assert!(msg.contains("`Parser`"));
/// ```
#[must_use]
pub fn format_primary_message(diagnostic: &BrainTraitDiagnostic) -> String {
    let name = diagnostic.trait_name();
    let total = diagnostic.total_method_count();
    let req = diagnostic.required_method_count();
    let def = diagnostic.default_method_count();
    let cc = diagnostic.default_method_cc_sum();

    if cc > 0 {
        format!(
            "`{name}` has {total} methods ({req} required, \
             {def} default) with default method complexity CC={cc}."
        )
    } else {
        format!("`{name}` has {total} methods ({req} required, {def} default).")
    }
}

/// Formats the note explaining what the metrics mean.
///
/// Surfaces method count as interface size, default method CC sum when
/// non-zero, and implementor burden when required methods are present.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitDiagnostic, BrainTraitDisposition, format_note,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let metrics = TraitMetricsBuilder::new("Foo").build();
/// let diag = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Pass);
/// let note = format_note(&diag);
/// assert!(note.contains("interface size"));
/// ```
#[must_use]
pub fn format_note(diagnostic: &BrainTraitDiagnostic) -> String {
    let mut note =
        String::from("Total method count measures interface size and implementation surface area.");
    if diagnostic.default_method_cc_sum() > 0 {
        note.push_str(
            " Default method CC sum measures complexity hidden behind \
             the trait's default implementations.",
        );
    }
    if diagnostic.required_method_count() > 0 {
        note.push_str(
            " Implementor burden indicates how many methods each \
             implementor must provide.",
        );
    }
    note
}

/// Formats a decomposition note from precomputed community suggestions.
///
/// Returns `None` when there are no suggestions to render.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitDiagnostic, BrainTraitDisposition, format_decomposition_note,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let metrics = TraitMetricsBuilder::new("Foo").build();
/// let diagnostic = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Pass);
///
/// assert_eq!(format_decomposition_note(&diagnostic, &[]), None);
/// ```
#[must_use]
pub fn format_decomposition_note(
    diagnostic: &BrainTraitDiagnostic,
    suggestions: &[DecompositionSuggestion],
) -> Option<String> {
    let context = DecompositionContext::new(diagnostic.trait_name(), SubjectKind::Trait);
    format_diagnostic_note(&context, suggestions)
}

/// Formats help text with tailored decomposition guidance.
///
/// The guidance varies based on the diagnostic signals:
/// - Many methods: suggests splitting into focused sub-traits.
/// - High default CC: suggests extracting complex defaults.
/// - High implementor burden: suggests providing more defaults.
/// - Fallback: general decomposition advice.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::evaluation::{
///     BrainTraitDiagnostic, BrainTraitDisposition, format_help,
/// };
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let metrics = TraitMetricsBuilder::new("Foo").build();
/// let diag = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Pass);
/// let help = format_help(&diag);
/// assert!(help.contains("splitting"));
/// ```
#[must_use]
pub fn format_help(diagnostic: &BrainTraitDiagnostic) -> String {
    let mut parts: Vec<&str> = Vec::new();

    if diagnostic.total_method_count() > 0 {
        parts.push("splitting the trait into focused sub-traits");
    }
    if diagnostic.default_method_cc_sum() > 0 {
        parts.push(
            "extracting complex default method bodies into free \
             functions or helper traits",
        );
    }
    if diagnostic.required_method_count() > 0 {
        parts.push("providing more default implementations to reduce implementor burden");
    }

    if parts.is_empty() {
        return String::from(
            "Consider splitting the trait into smaller, focused \
             sub-traits to reduce complexity.",
        );
    }

    format!("Consider {}.", parts.join(", "))
}
