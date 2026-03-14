//! Diagnostic-note rendering for decomposition suggestions.
//!
//! This module keeps the developer-facing wording for decomposition advice in
//! one place so `brain_type` and `brain_trait` stay aligned. Rendering is
//! deterministic and compiler independent, which keeps it suitable for reuse
//! from future lint drivers and localisation layers.

use super::{DecompositionContext, DecompositionSuggestion};

#[cfg(test)]
#[path = "note_tests.rs"]
mod tests;

const MAX_SUGGESTIONS: usize = 3;
const MAX_METHODS_PER_SUGGESTION: usize = 3;

/// Formats a concise diagnostic note for decomposition suggestions.
///
/// Returns `None` when there are no suggestions to render. Otherwise the note
/// is multi-line, starts with the analysed subject name, and caps both the
/// number of displayed suggestion areas and the number of displayed methods
/// per area.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::{
///     DecompositionContext, MethodProfileBuilder, SubjectKind, format_diagnostic_note,
///     suggest_decomposition,
/// };
///
/// let context = DecompositionContext::new("Store", SubjectKind::Type);
///
/// let mut load = MethodProfileBuilder::new("load");
/// load.record_external_domain("std::fs");
///
/// let mut save = MethodProfileBuilder::new("save");
/// save.record_external_domain("std::fs");
///
/// let mut encode = MethodProfileBuilder::new("encode");
/// encode.record_external_domain("serde::json");
///
/// let mut decode = MethodProfileBuilder::new("decode");
/// decode.record_external_domain("serde::json");
///
/// let note = format_diagnostic_note(
///     &context,
///     &suggest_decomposition(
///         &context,
///         &[load.build(), save.build(), encode.build(), decode.build()],
///     ),
/// );
///
/// assert!(note.is_some());
/// assert!(note.unwrap_or_default().contains("Potential decomposition"));
/// ```
#[must_use]
pub fn format_diagnostic_note(
    context: &DecompositionContext,
    suggestions: &[DecompositionSuggestion],
) -> Option<String> {
    if suggestions.is_empty() {
        return None;
    }

    let visible_suggestions = &suggestions[..suggestions.len().min(MAX_SUGGESTIONS)];
    let mut lines = vec![format!(
        "Potential decomposition for `{}`:",
        context.subject_name()
    )];

    lines.extend(visible_suggestions.iter().map(render_suggestion_line));

    let omitted_suggestions = suggestions.len().saturating_sub(MAX_SUGGESTIONS);
    if omitted_suggestions > 0 {
        lines.push(format!("{omitted_suggestions} more areas omitted"));
    }

    Some(lines.join("\n"))
}

fn render_suggestion_line(suggestion: &DecompositionSuggestion) -> String {
    let rendered_methods = render_method_list(suggestion.methods());
    format!(
        "- [{}] {} for {}",
        suggestion.label(),
        suggestion.extraction_kind(),
        rendered_methods
    )
}

fn render_method_list(methods: &[String]) -> String {
    let visible_methods = &methods[..methods.len().min(MAX_METHODS_PER_SUGGESTION)];
    let mut rendered = visible_methods
        .iter()
        .map(|method| format!("`{method}`"))
        .collect::<Vec<_>>()
        .join(", ");

    let omitted_methods = methods.len().saturating_sub(MAX_METHODS_PER_SUGGESTION);
    if omitted_methods > 0 {
        rendered.push_str(&format!(", +{omitted_methods} more methods"));
    }

    rendered
}
