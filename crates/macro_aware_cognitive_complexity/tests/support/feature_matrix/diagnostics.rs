//! Diagnostic extraction and normalization for the feature-graph matrix.
//!
//! Each run's `compiler-message` entries are reduced to one deterministic line
//! per diagnostic. Only the generated project root is replaced with
//! `<PROJECT>`; scores, spans, message text, and counts are never normalized.

use anyhow::Context as _;
use camino::Utf8Path;
use serde_json::Value;

use super::{PROJECT_SOURCE, ProjectMode, runs::ProjectRun};
use crate::support::{LINT_CODE, RawRun, compiler_messages, parse_score};

/// One normalized lint diagnostic, comparable across generated projects.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NormalizedDiagnostic {
    /// The span and rendered message joined into one deterministic line.
    pub(crate) line: String,
    /// The rendered message, from which the score is parsed.
    pub(crate) message: String,
}

/// Collect this lint's diagnostics with each project root replaced by `<PROJECT>`.
///
/// # Errors
///
/// Return an error when a selected diagnostic lacks a primary span, a numeric
/// span field, or a rendered message.
pub(crate) fn collect_normalized(
    raw: &RawRun,
    project_root: &Utf8Path,
) -> anyhow::Result<Vec<NormalizedDiagnostic>> {
    let mut diagnostics = Vec::new();
    for message in compiler_messages(raw) {
        let diagnostic = message
            .get("message")
            .context("compiler message has no `message` field")?;
        let is_under_test = diagnostic
            .get("code")
            .and_then(|entry| entry.get("code"))
            .and_then(Value::as_str)
            == Some(LINT_CODE);
        if !is_under_test {
            continue;
        }
        let span = primary_span(diagnostic)?;
        let rendered = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .context("diagnostic has no rendered message")?;
        let file_name = span
            .get("file_name")
            .and_then(Value::as_str)
            .context("primary span has no `file_name`")?;
        let normalized_file = normalize_file_name(file_name, project_root);
        let line = format!(
            "{normalized_file}:{}:{}:{}:{}: {rendered}",
            span_field(span, "line_start")?,
            span_field(span, "line_end")?,
            span_field(span, "column_start")?,
            span_field(span, "column_end")?,
        );
        diagnostics.push(NormalizedDiagnostic {
            line,
            message: rendered.to_owned(),
        });
    }
    Ok(diagnostics)
}

/// Return the diagnostic's primary span.
fn primary_span(diagnostic: &Value) -> anyhow::Result<&Value> {
    diagnostic
        .get("spans")
        .and_then(Value::as_array)
        .and_then(|spans| {
            spans
                .iter()
                .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
        })
        .context("diagnostic has no primary span")
}

/// Return one numeric span field.
fn span_field(span: &Value, field: &str) -> anyhow::Result<u64> {
    span.get(field)
        .and_then(Value::as_u64)
        .with_context(|| format!("primary span has no `{field}`"))
}

/// Replace only the project root prefix in `file_name` with `<PROJECT>`.
///
/// A relative `file_name` carries no root, so it is left untouched; nothing
/// else in the path, span, text, or count is normalized.
fn normalize_file_name(file_name: &str, project_root: &Utf8Path) -> String {
    file_name.strip_prefix(project_root.as_str()).map_or_else(
        || file_name.to_owned(),
        |remainder| format!("<PROJECT>{remainder}"),
    )
}

/// Assert one project produced exactly one normalized diagnostic.
pub(crate) fn assert_single_diagnostic(diagnostics: &[NormalizedDiagnostic], mode: ProjectMode) {
    assert_eq!(
        diagnostics.len(),
        1,
        "{} must report exactly one diagnostic, found {}",
        mode.label(),
        diagnostics.len()
    );
}

/// Assert both projects produced the same, non-empty normalized diagnostics.
pub(crate) fn assert_vectors_match(
    plain: &[NormalizedDiagnostic],
    with_log: &[NormalizedDiagnostic],
) {
    assert!(
        !plain.is_empty(),
        "the plain project produced no normalized diagnostics"
    );
    assert!(
        !with_log.is_empty(),
        "the `log` project produced no normalized diagnostics"
    );
    assert_eq!(
        plain, with_log,
        "the `log` feature must not change the normalized diagnostics"
    );
}

/// Parse the single diagnostic's score from a normalized vector.
///
/// # Errors
///
/// Return an error when the vector is empty or the message carries no score.
pub(crate) fn single_score(diagnostics: &[NormalizedDiagnostic]) -> anyhow::Result<u64> {
    let diagnostic = diagnostics
        .first()
        .context("expected one diagnostic to score")?;
    parse_score(&diagnostic.message).context("could not parse a score from the diagnostic")
}

/// Assert the reported complexity is the authored baseline of one.
pub(crate) fn assert_baseline_score(score: u64) {
    assert_eq!(
        score, 1,
        "four generated logging calls must not add authored complexity, got {score}"
    );
}

/// Assert the generated source contains exactly four `tracing::info!` calls.
pub(crate) fn assert_four_logging_calls() {
    assert_eq!(
        PROJECT_SOURCE.matches("tracing::info!").count(),
        4,
        "the generated source must contain exactly four `tracing::info!` calls"
    );
}

/// Assert the feature graph is exactly the intended difference.
///
/// Both the `log` package and `tracing`'s `log` feature edge are checked. The
/// matrix adds `log` to the manifest as well as enabling the feature, so
/// package presence alone would still hold if the feature were dropped and the
/// two projects compiled the same expansion.
pub(crate) fn assert_log_feature_graph(plain: &ProjectRun, with_log: &ProjectRun) {
    assert!(
        !plain.tree_lists_log,
        "the plain project's `cargo tree -e features` must not list `log`"
    );
    assert!(
        !plain.tree_lists_tracing_log_feature,
        "the plain project's `cargo tree -e features` must not enable `tracing`'s `log` feature"
    );
    assert!(
        with_log.tree_lists_log,
        "the `log` project's `cargo tree -e features` must list `log`"
    );
    assert!(
        with_log.tree_lists_tracing_log_feature,
        "the `log` project's `cargo tree -e features` must enable `tracing`'s `log` feature"
    );
}
