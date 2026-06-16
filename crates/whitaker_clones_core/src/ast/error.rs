//! Typed errors for AST lowering and feature extraction.

/// Result alias for AST feature extraction operations.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::AstResult;
///
/// fn ok() -> AstResult<()> { Ok(()) }
///
/// assert!(ok().is_ok());
/// ```
pub type AstResult<T> = Result<T, AstError>;

/// Error cases reported while validating or lowering AST candidate spans.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::AstError;
///
/// let error = AstError::EmptySpan { offset: 4 };
/// assert_eq!(error.to_string(), "byte span is empty at offset 4");
/// ```
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AstError {
    /// The span end precedes the span start.
    #[error("byte span end {end} precedes start {start}")]
    InvalidSpan {
        /// Start byte offset.
        start: u32,
        /// End byte offset.
        end: u32,
    },
    /// The span covers no bytes.
    #[error("byte span is empty at offset {offset}")]
    EmptySpan {
        /// Empty span offset.
        offset: u32,
    },
    /// The span boundary does not align with a UTF-8 character boundary.
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    NonCharBoundary {
        /// Invalid byte boundary.
        offset: u32,
    },
    /// The span lies outside the parsed source.
    #[error("byte span {start}..{end} lies outside the parsed source of length {len}")]
    SpanOutOfBounds {
        /// Start byte offset.
        start: u32,
        /// End byte offset.
        end: u32,
        /// Source text byte length.
        len: usize,
    },
    /// A byte offset cannot be represented by the parser's text-size type.
    #[error("byte offset {0} exceeds the u32 TextSize range")]
    OffsetTooLarge(u32),
    /// The selected subtree is dominated by parse-error nodes.
    #[error("byte span {start}..{end} maps to an unparsable (ERROR) subtree")]
    UnparsableSpan {
        /// Start byte offset.
        start: u32,
        /// End byte offset.
        end: u32,
    },
}
