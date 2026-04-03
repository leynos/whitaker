//! Diagnostic detail and formatting for brain type evaluation.
//!
//! Carries measured values and disposition after threshold evaluation,
//! and provides formatting functions that produce human-readable
//! diagnostic messages for the lint driver.
//!
//! See `docs/brain-trust-lints-design.md` §Diagnostic output for the
//! full format specification.

use std::fmt::Write;

use super::evaluation::BrainTypeDisposition;
use super::{MethodMetrics, TypeMetrics};
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
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeDiagnostic, BrainTypeDisposition,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let metrics = TypeMetricsBuilder::new("Foo", 25, 80).build();
/// let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
/// assert_eq!(diag.type_name(), "Foo");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainTypeDiagnostic {
    type_name: String,
    disposition: BrainTypeDisposition,
    wmc: usize,
    lcom4: usize,
    foreign_reach: usize,
    brain_methods: Vec<MethodMetrics>,
}

impl BrainTypeDiagnostic {
    /// Creates a diagnostic from evaluated metrics and disposition.
    #[must_use]
    pub fn new(metrics: &TypeMetrics, disposition: BrainTypeDisposition) -> Self {
        Self {
            type_name: metrics.type_name().to_owned(),
            disposition,
            wmc: metrics.wmc(),
            lcom4: metrics.lcom4(),
            foreign_reach: metrics.foreign_reach(),
            brain_methods: metrics.brain_methods().to_vec(),
        }
    }

    /// Returns the type name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the evaluation disposition.
    #[must_use]
    pub fn disposition(&self) -> BrainTypeDisposition {
        self.disposition
    }

    /// Returns the Weighted Methods Count.
    #[must_use]
    pub fn wmc(&self) -> usize {
        self.wmc
    }

    /// Returns the LCOM4 connected component count.
    #[must_use]
    pub fn lcom4(&self) -> usize {
        self.lcom4
    }

    /// Returns the foreign reach count.
    #[must_use]
    pub fn foreign_reach(&self) -> usize {
        self.foreign_reach
    }

    /// Returns brain methods with their full metric details.
    #[must_use]
    pub fn brain_methods(&self) -> &[MethodMetrics] {
        &self.brain_methods
    }
}

// ---------------------------------------------------------------------------
// Diagnostic formatting
// ---------------------------------------------------------------------------

/// Formats the primary diagnostic message with measured values.
///
/// Includes foreign reach when non-zero. The message varies based on
/// brain method count:
/// - 0 brain methods: reports WMC and LCOM4.
/// - 1 brain method: names the method with its CC and LOC.
/// - 2+ brain methods: lists each method with its CC and LOC.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeDiagnostic, BrainTypeDisposition, format_primary_message,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
/// builder.add_method("parse", 31, 140);
/// builder.set_lcom4(3);
/// let metrics = builder.build();
/// let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Warn);
/// let msg = format_primary_message(&diag);
/// assert!(msg.contains("WMC=31"));
/// assert!(msg.contains("LCOM4=3"));
/// assert!(msg.contains("parse"));
/// ```
#[must_use]
pub fn format_primary_message(diagnostic: &BrainTypeDiagnostic) -> String {
    match diagnostic.brain_methods() {
        [] => format_primary_without_brain_methods(diagnostic),
        [bm] => format_primary_with_one_brain_method(diagnostic, bm),
        methods => format_primary_with_many_brain_methods(diagnostic, methods),
    }
}

/// Formats the primary message when no brain methods are present.
fn format_primary_without_brain_methods(diagnostic: &BrainTypeDiagnostic) -> String {
    let name = diagnostic.type_name();
    let wmc = diagnostic.wmc();
    let lcom4 = diagnostic.lcom4();
    let fr_suffix = foreign_reach_suffix(diagnostic);
    format!("`{name}` has WMC={wmc} and LCOM4={lcom4}{fr_suffix}.")
}

/// Formats the primary message when exactly one brain method is present.
fn format_primary_with_one_brain_method(
    diagnostic: &BrainTypeDiagnostic,
    bm: &MethodMetrics,
) -> String {
    let name = diagnostic.type_name();
    let wmc = diagnostic.wmc();
    let lcom4 = diagnostic.lcom4();
    let fr_suffix = foreign_reach_suffix(diagnostic);
    format!(
        "`{name}` has WMC={wmc}, LCOM4={lcom4}{fr_suffix}, \
         and a brain method `{}` (CC={}, LOC={}).",
        bm.name(),
        bm.cognitive_complexity(),
        bm.lines_of_code(),
    )
}

/// Formats the primary message when multiple brain methods are present.
fn format_primary_with_many_brain_methods(
    diagnostic: &BrainTypeDiagnostic,
    methods: &[MethodMetrics],
) -> String {
    let name = diagnostic.type_name();
    let wmc = diagnostic.wmc();
    let lcom4 = diagnostic.lcom4();
    let fr_suffix = foreign_reach_suffix(diagnostic);
    let n = methods.len();
    let mut msg = format!(
        "`{name}` has WMC={wmc}, LCOM4={lcom4}{fr_suffix}, \
         and {n} brain methods: ",
    );
    for (i, bm) in methods.iter().enumerate() {
        if i > 0 {
            msg.push_str(", ");
        }
        // Write cannot fail on String.
        let _ = write!(
            msg,
            "`{}` (CC={}, LOC={})",
            bm.name(),
            bm.cognitive_complexity(),
            bm.lines_of_code(),
        );
    }
    msg.push('.');
    msg
}

/// Returns the foreign reach suffix for the primary message, or an
/// empty string when foreign reach is zero.
fn foreign_reach_suffix(diagnostic: &BrainTypeDiagnostic) -> String {
    let fr = diagnostic.foreign_reach();
    if fr > 0 {
        format!(", foreign reach={fr}")
    } else {
        String::new()
    }
}

/// Formats the note explaining what the metrics mean.
///
/// Surfaces foreign reach when it is non-zero, alongside WMC, brain
/// method, and LCOM4 explanations.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeDiagnostic, BrainTypeDisposition, format_note,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let metrics = TypeMetricsBuilder::new("Foo", 25, 80).build();
/// let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
/// let note = format_note(&diag);
/// assert!(note.contains("WMC"));
/// ```
#[must_use]
pub fn format_note(diagnostic: &BrainTypeDiagnostic) -> String {
    let mut note = String::from("WMC measures total cognitive complexity across all methods.");
    if !diagnostic.brain_methods().is_empty() {
        note.push_str(" Brain methods are methods with high complexity and size.");
    }
    if diagnostic.lcom4() >= 2 {
        note.push_str(
            " LCOM4 >= 2 indicates the type has multiple unrelated \
             responsibilities.",
        );
    }
    if diagnostic.foreign_reach() > 0 {
        let _ = write!(
            note,
            " Foreign reach of {} indicates coupling to external modules.",
            diagnostic.foreign_reach(),
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
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeDiagnostic, BrainTypeDisposition, format_decomposition_note,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let metrics = TypeMetricsBuilder::new("Foo", 25, 80).build();
/// let diagnostic = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
///
/// assert_eq!(format_decomposition_note(&diagnostic, &[]), None);
/// ```
#[must_use]
pub fn format_decomposition_note(
    diagnostic: &BrainTypeDiagnostic,
    suggestions: &[DecompositionSuggestion],
) -> Option<String> {
    let context = DecompositionContext::new(diagnostic.type_name(), SubjectKind::Type);
    format_diagnostic_note(&context, suggestions)
}

/// Formats help text with tailored decomposition guidance.
///
/// The guidance varies based on the diagnostic signals:
/// - Brain methods present: suggests extracting or simplifying them.
/// - High LCOM4: suggests splitting unrelated responsibilities.
/// - High foreign reach: suggests reducing external coupling.
/// - Default: general decomposition advice.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_type_metrics::evaluation::{
///     BrainTypeDiagnostic, BrainTypeDisposition, format_help,
/// };
/// use whitaker_common::brain_type_metrics::TypeMetricsBuilder;
///
/// let metrics = TypeMetricsBuilder::new("Foo", 25, 80).build();
/// let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
/// let help = format_help(&diag);
/// assert!(help.contains("extract"));
/// ```
#[must_use]
pub fn format_help(diagnostic: &BrainTypeDiagnostic) -> String {
    let mut parts: Vec<&str> = Vec::new();

    if !diagnostic.brain_methods().is_empty() {
        parts.push("extracting or simplifying brain methods");
    }
    if diagnostic.lcom4() >= 2 {
        parts.push("splitting unrelated responsibilities into separate types");
    }
    if diagnostic.foreign_reach() > 0 {
        parts.push("reducing coupling to external modules");
    }

    if parts.is_empty() {
        return String::from(
            "Consider extracting related methods into separate types or \
             modules to reduce complexity and improve cohesion.",
        );
    }

    format!("Consider {}.", parts.join(", "))
}
