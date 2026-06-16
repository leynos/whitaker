//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use super::{AstError, AstResult, ByteSpan, NormalisedTree};

/// Parser schema version mixed into AST hashes.
///
/// Stage B replaces this neutral skeleton value with the exact parser pin.
pub const PARSER_SCHEMA_VERSION: &str = "0.0.PINNED";

/// Parses `file_text`, maps `span` to the smallest covering syntax node, and
/// lowers that subtree into a [`NormalisedTree`].
///
/// Stage D supplies the parser-backed implementation. Until then this returns
/// a typed placeholder error after validating that the span is representable.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{AstError, ByteSpan, lower_span};
///
/// let span = ByteSpan::new("fn f() {}", 0, 2)?;
/// assert_eq!(
///     lower_span("fn f() {}", span),
///     Err(AstError::UnparsableSpan { start: 0, end: 2 }),
/// );
/// # Ok::<(), AstError>(())
/// ```
pub fn lower_span(_file_text: &str, span: ByteSpan) -> AstResult<NormalisedTree> {
    Err(AstError::UnparsableSpan {
        start: span.start(),
        end: span.end(),
    })
}
