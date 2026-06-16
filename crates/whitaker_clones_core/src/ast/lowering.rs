//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use super::{AstError, AstResult, ByteSpan, NormalisedTree};

pub use crate::hashing::PARSER_SCHEMA_VERSION;

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

#[cfg(test)]
mod tests {
    use ra_ap_syntax::{AstNode, Edition, SourceFile};

    #[test]
    fn pinned_parser_snapshot_parses_current_edition_source() {
        let parse = SourceFile::parse("fn f() {}", Edition::CURRENT);

        assert!(parse.errors().is_empty(), "{:?}", parse.errors());
        assert!(!parse.tree().syntax().text_range().is_empty());
    }
}
