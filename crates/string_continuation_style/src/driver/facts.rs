//! Extracts exact, editable rewrites from plain cooked string expressions.
//!
//! This is the compiler boundary: cheap AST facts and the positive wrapper
//! depth set gate source recovery, then the pure domain crate classifies and
//! constructs the replacement. Any uncertain source fact yields no finding.

use std::{collections::HashSet, env};

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use log::debug;
use rustc_ast::ast;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{NormalizedPos, Pos, Span};
use whitaker_string_literals::{
    classification::{ContinuationDisposition, classify},
    facts::{LiteralContext, LiteralFacts},
    rewrite::{Newline, cooked_literal_body, gate::preserves_value},
};

use super::{literal::plain_join_literal, span::body_range_span};
use crate::config::LINT_NAME;

/// A validated replacement and the continuation spans it will label.
pub(crate) struct LiteralRewrite {
    /// Whole source-literal token replaced by the suggestion.
    pub(crate) literal_span: Span,
    /// One span for each source-line join in the literal.
    pub(crate) join_spans: Vec<Span>,
    /// Replacement expression preserving the decoded literal value.
    pub(crate) replacement: String,
}

/// Extracts a rewrite when `expression` is a safe, editable source literal.
pub(crate) fn plain_literal_rewrite(
    cx: &EarlyContext<'_>,
    expression: &ast::Expr,
    safe_literal_spans: &HashSet<Span>,
) -> Option<LiteralRewrite> {
    let literal = plain_join_literal(expression)?;

    if expression.span.from_expansion() || !safe_literal_spans.contains(&expression.span) {
        return None;
    }

    source_literal_rewrite(cx, expression.span, literal.symbol.as_str())
}

fn source_literal_rewrite(
    cx: &EarlyContext<'_>,
    literal_span: Span,
    uncooked_body: &str,
) -> Option<LiteralRewrite> {
    let source_map = cx.sess().source_map();
    let source_path = source_map
        .span_to_filename(literal_span)
        .into_local_path()?;
    let source_path = Utf8PathBuf::try_from(source_path).ok()?;
    let source_file = source_map.lookup_source_file(literal_span.lo());
    let source = source_file.src.as_deref()?;
    let newline = dominant_newline(source, &source_file.normalized_pos);
    let snippet = source_map.span_to_snippet(literal_span).ok()?;
    let body = cooked_literal_body(&snippet)?;
    if body.as_str() != uncooked_body {
        return None;
    }

    let context = if is_editable_source(source_path) {
        LiteralContext::Expression
    } else {
        LiteralContext::GeneratedSourceFile
    };
    let facts = LiteralFacts::Cooked { body, context };
    let rewrite = match classify(&facts) {
        ContinuationDisposition::PreferConcat(rewrite) => {
            debug!(target: LINT_NAME, "literal classified for a concat rewrite");
            rewrite
        }
        ContinuationDisposition::RequireContinuation(reason) => {
            debug!(target: LINT_NAME, "literal requires its continuation: {reason:?}");
            return None;
        }
        ContinuationDisposition::Ignore => {
            debug!(target: LINT_NAME, "literal ignored by the continuation policy");
            return None;
        }
    };
    if !preserves_value(body.as_str(), rewrite.fragments()) {
        debug!(target: LINT_NAME, "runtime value check withheld the rewrite");
        return None;
    }

    let join_spans = rewrite
        .join_offsets()
        .iter()
        .map(|offset| {
            let start = offset.get();
            let end = start.checked_add(2)?;
            body_range_span(
                literal_span,
                // Keep all body-relative to token-relative conversion in
                // `span.rs`, including its quote and bounds checks.
                body.len(),
                start..end,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let indent = source_map.lookup_char_pos(literal_span.lo()).col.to_usize();

    Some(LiteralRewrite {
        literal_span,
        join_spans,
        replacement: rewrite.render(indent, newline),
    })
}

fn is_editable_source(source_path: Utf8PathBuf) -> bool {
    let Some(workspace_root) = env::current_dir()
        .ok()
        .and_then(|path| Utf8PathBuf::try_from(path).ok())
    else {
        return false;
    };
    let source_path = absolute_path(&workspace_root, source_path);

    let configured_target_directory = match env::var("CARGO_TARGET_DIR") {
        Ok(path) => Utf8PathBuf::from(path),
        Err(env::VarError::NotPresent) => workspace_root.join("target"),
        Err(env::VarError::NotUnicode(_)) => return false,
    };
    let configured_target_directory = absolute_path(&workspace_root, configured_target_directory);
    source_path.starts_with(&workspace_root)
        && !source_path.starts_with(configured_target_directory)
        && !source_path.starts_with(workspace_root.join("target"))
}

fn absolute_path(root: &Utf8Path, path: Utf8PathBuf) -> Utf8PathBuf {
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let mut normalized = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::Prefix(_) | Utf8Component::RootDir => {
                normalized.push(component.as_str());
            }
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                normalized.pop();
            }
            Utf8Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

fn dominant_newline(source: &str, normalized_positions: &[NormalizedPos]) -> Newline {
    let line_feeds = source.bytes().filter(|byte| *byte == b'\n').count();
    let removed_bytes = normalized_positions
        .last()
        .map_or(0, |position| position.diff);
    let removed_bom = normalized_positions
        .first()
        .is_some_and(|position| position.pos.0 == 0 && position.diff == 3);
    let carriage_return_line_feeds = usize::try_from(removed_bytes)
        .unwrap_or(usize::MAX)
        .saturating_sub(if removed_bom { 3 } else { 0 });
    let original_line_feeds = line_feeds.saturating_sub(carriage_return_line_feeds);

    if carriage_return_line_feeds > original_line_feeds {
        Newline::CrLf
    } else {
        Newline::Lf
    }
}

#[cfg(test)]
#[path = "../../tests/support/facts_tests.rs"]
mod tests;
