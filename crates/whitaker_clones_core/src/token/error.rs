//! Error types for token normalization and fingerprint generation.

use thiserror::Error;

/// Result alias for token-pass operations.
pub type Result<T> = std::result::Result<T, TokenPassError>;

/// Errors raised while normalizing or fingerprinting token streams.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TokenPassError {
    /// `k` must be greater than zero for shingling.
    #[error("shingle size must be greater than zero")]
    ZeroShingleSize,
    /// The winnowing window must be greater than zero.
    #[error("winnow window must be greater than zero")]
    ZeroWinnowWindow,
    /// The lexer emitted an unknown token that cannot be normalized.
    #[error("unsupported token at byte range {start}..{end}")]
    UnsupportedToken { start: usize, end: usize },
    /// The source ended before a block comment was terminated.
    #[error("unterminated block comment at byte range {start}..{end}")]
    UnterminatedBlockComment { start: usize, end: usize },
    /// The source ended before a literal token was terminated.
    #[error("unterminated {literal_kind} literal at byte range {start}..{end}")]
    UnterminatedLiteral {
        /// Human-readable literal category.
        literal_kind: &'static str,
        /// Inclusive start byte.
        start: usize,
        /// Exclusive end byte.
        end: usize,
    },
}
