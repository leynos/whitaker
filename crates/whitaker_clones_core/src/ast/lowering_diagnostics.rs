//! Diagnostics emitted by the AST lowering adapter.
//!
//! Most `tracing` events the adapter produces are funnelled through this
//! module. The functions that decide an outcome live in [`super::lowering`] and
//! stay free of `tracing` macros on purpose: those macros expand into
//! conditional code, and Clippy's cognitive-complexity lint counts the
//! expansion. A budget check that both decides and reports therefore reads as
//! far more complex than it is, which obscures the decision it makes.
//! Reporting is a separate step with no influence on control flow, so it
//! belongs here.
//!
//! Unlike [`super::lowering`], this module must not name a parser crate type.
//! The `ast_boundary` test admits exactly one adapter module and treats every
//! other file under `src/ast` as domain code, so the events reported here are
//! described with the primitives [`super::lowering`] has already extracted.

use tracing::{error, warn};

use super::{AstError, ByteSpan};

/// Log a failed span re-validation and return `error` unchanged.
pub(super) fn log_span_validation_failure(error: AstError) -> AstError {
    trace_ast_error(
        error,
        "AST span lies outside the supplied source text",
        "AST span validation failed",
    )
}

/// Log a failed covering-node selection and return `error` unchanged.
pub(super) fn log_covering_failure(error: AstError) -> AstError {
    trace_ast_error(
        error,
        "no AST syntax node covers the requested span",
        "AST covering-node selection failed",
    )
}

/// Log a lowering failure, naming parser error elements when that is the cause.
pub(super) fn log_lowering_failure(span: ByteSpan, error: AstError) -> AstError {
    if matches!(error, AstError::UnparsableSpan { .. }) {
        log_unparsable_span(span);
    }
    error
}

/// Report parser recovery errors that were tolerated before lowering.
pub(super) fn log_parser_recovery(span: ByteSpan, error_count: usize) {
    warn!(
        start = span.start(),
        end = span.end(),
        errors = error_count,
        "lowered AST span from source with parser recovery errors"
    );
}

/// Report that covering-node selection exceeded its depth budget.
pub(super) fn log_selection_depth_budget(span: ByteSpan, depth: usize, limit: usize) {
    error!(
        start = span.start(),
        end = span.end(),
        depth,
        limit,
        "AST covering-node selection exceeded the depth budget"
    );
}

/// Report that covering-node selection exceeded its node budget.
pub(super) fn log_selection_node_budget(span: ByteSpan, node_count: usize, limit: usize) {
    error!(
        start = span.start(),
        end = span.end(),
        node_count,
        limit,
        "AST covering-node selection exceeded the node budget"
    );
}

/// Report that lowering exceeded its depth budget.
pub(super) fn log_lowering_depth_budget(span: ByteSpan, depth: usize, limit: usize) {
    error!(
        start = span.start(),
        end = span.end(),
        depth,
        limit,
        "AST lowering exceeded the depth budget"
    );
}

/// Report that lowering exceeded its normalized-node budget.
pub(super) fn log_lowering_node_budget(span: ByteSpan, count: usize, limit: usize) {
    error!(
        start = span.start(),
        end = span.end(),
        normalized_node_count = count,
        limit,
        "AST lowering exceeded the normalized-node budget"
    );
}

/// Report that the selected subtree contains parser error elements.
fn log_unparsable_span(span: ByteSpan) {
    error!(
        start = span.start(),
        end = span.end(),
        "selected AST span contains parser error elements"
    );
}

/// Log a lowering failure and return it unchanged.
///
/// `AstError::SpanOutOfBounds` gets the caller's span-specific message and
/// structured offsets because it is the only variant callers disambiguate by
/// site; every other failure reports the error itself.
fn trace_ast_error(
    error: AstError,
    span_out_of_bounds_message: &'static str,
    generic_message: &'static str,
) -> AstError {
    if let AstError::SpanOutOfBounds { start, end, len } = &error {
        log_out_of_bounds(*start, *end, *len, span_out_of_bounds_message);
    } else {
        log_generic_failure(&error, generic_message);
    }
    error
}

/// Report a span that lies outside the supplied source text.
fn log_out_of_bounds(start: u32, end: u32, len: usize, message: &'static str) {
    error!(start, end, len, "{message}");
}

/// Report any other lowering failure.
fn log_generic_failure(error: &AstError, message: &'static str) {
    error!(?error, "{message}");
}
