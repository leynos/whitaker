//! Extracts exact, editable rewrites from plain cooked string expressions.
//!
//! This is the compiler boundary: cheap AST facts and the positive wrapper
//! depth set gate source recovery, then the pure domain crate classifies and
//! constructs the replacement. Any uncertain source fact yields no finding.

use std::collections::HashSet;

use camino::Utf8PathBuf;
use log::debug;
use rustc_ast::ast;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{NormalizedPos, Pos, Span};
use whitaker_string_literals::{
    classification::{ContinuationDisposition, classify},
    facts::{CaptureStatus, LiteralContext, LiteralFacts},
    rewrite::{Newline, cooked_literal_body, gate::preserves_value},
};

use super::{
    edit_policy::SourceEditPolicy,
    literal::{
        has_captured_argument,
        join_format_string,
        outermost_macro_name,
        plain_join_literal,
    },
    shadow::is_format_macro_name,
    span::body_range_span,
};
use crate::config::LINT_NAME;

/// Host services every rewrite query needs.
///
/// Bundled so adding a host fact does not change each query's arity, and so a
/// query cannot accidentally pair one caller's compiler context with another's
/// policy.
pub(crate) struct SourceContext<'cx, 'tcx> {
    /// Compiler context used for source lookup and span resolution.
    pub(crate) cx: &'cx EarlyContext<'tcx>,
    /// Workspace policy deciding whether a literal's file may be rewritten.
    pub(crate) edit_policy: &'cx SourceEditPolicy,
}

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
    context: &SourceContext<'_, '_>,
    expression: &ast::Expr,
    safe_literal_spans: &HashSet<Span>,
) -> Option<LiteralRewrite> {
    let literal = plain_join_literal(expression)?;

    if expression.span.from_expansion() || !safe_literal_spans.contains(&expression.span) {
        return None;
    }

    source_literal_rewrite(
        context,
        expression.span,
        literal.symbol.as_str(),
        |is_editable| {
            if is_editable {
                LiteralContext::Expression
            } else {
                LiteralContext::GeneratedSourceFile
            }
        },
    )
}

/// Extracts a rewrite when `expression` is a safe, editable format string
/// reached through one of the RFC's format macros.
///
/// Two different spans matter here, and they are not interchangeable. The
/// rewrite replaces `arguments.span`, which the builtin sets to the literal
/// token; the enclosing expression's span is def-site-marked by the builtin.
/// The macro-identity test needs that second span: a `println!`-family literal
/// travels through `$($arg:tt)*` unchanged, so the token keeps root context
/// and its own `source_callee()` is empty, while the def-site-marked
/// expression's chain reaches the macro the user typed.
pub(crate) fn format_args_rewrite(
    context: &SourceContext<'_, '_>,
    expression: &ast::Expr,
    arguments: &ast::FormatArgs,
    safe_format_spans: &HashSet<Span>,
) -> Option<LiteralRewrite> {
    let uncooked_body = join_format_string(arguments)?;

    if !safe_format_spans.contains(&arguments.span) {
        return None;
    }
    let macro_name = outermost_macro_name(expression.span)?;
    if !is_format_macro_name(macro_name.as_str()) {
        return None;
    }
    let capture = if has_captured_argument(arguments) {
        CaptureStatus::Present
    } else {
        CaptureStatus::NoneObserved
    };

    source_literal_rewrite(context, arguments.span, uncooked_body, |is_editable| {
        if is_editable {
            LiteralContext::SourceFormatString { capture }
        } else {
            LiteralContext::GeneratedSourceFile
        }
    })
}

/// Turns a source literal span into a validated rewrite, if one is safe.
///
/// The `context_of` callback receives the editability answer rather than the
/// path, so callers cannot disagree with the policy about what "generated"
/// means. Every failure along the way returns `None`: a literal the lint
/// cannot read, re-lex, or prove value-preserving is silently skipped, which
/// is the same outcome as a literal with no join.
fn source_literal_rewrite(
    context: &SourceContext<'_, '_>,
    literal_span: Span,
    uncooked_body: &str,
    context_of: impl FnOnce(bool) -> LiteralContext,
) -> Option<LiteralRewrite> {
    let source_map = context.cx.sess().source_map();
    let source_path = source_map
        .span_to_filename(literal_span)
        .into_local_path()?;
    let utf8_source_path = Utf8PathBuf::try_from(source_path).ok()?;
    let source_file = source_map.lookup_source_file(literal_span.lo());
    let source = source_file.src.as_deref()?;
    let newline = dominant_newline(source, &source_file.normalized_pos);
    let snippet = source_map.span_to_snippet(literal_span).ok()?;
    let body = cooked_literal_body(&snippet)?;
    if body.as_str() != uncooked_body {
        return None;
    }

    let literal_context = context_of(context.edit_policy.is_editable(&utf8_source_path));
    let facts = LiteralFacts::Cooked {
        body,
        context: literal_context,
    };
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

/// Picks the newline style a rendered replacement should use.
///
/// A mixed file is resolved by majority, and the compiler's normalization map
/// is used rather than counting raw bytes: `\r\n` has already been rewritten
/// by the time the source is readable, so the removed bytes are what say how
/// many line feeds were CRLF in the original file. A byte-order mark is
/// subtracted first so it is not miscounted as part of a line ending.
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
