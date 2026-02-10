//! Error types for artefact naming, manifest, and verification policy.
//!
//! Each variant provides a descriptive message identifying the invalid input
//! and the constraint that was violated.

use thiserror::Error;

/// Errors arising from invalid artefact-related values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArtefactError {
    /// The target triple is not in the supported set.
    #[error("unsupported target triple \"{value}\"; expected one of: {expected}")]
    UnsupportedTarget {
        /// The rejected triple string.
        value: String,
        /// Comma-separated list of accepted triples.
        expected: String,
    },

    /// A toolchain channel string is empty or syntactically invalid.
    #[error("invalid toolchain channel: {reason}")]
    InvalidToolchainChannel {
        /// Description of the validation failure.
        reason: String,
    },

    /// A git SHA is empty, too long, or contains non-hex characters.
    #[error("invalid git SHA \"{value}\": {reason}")]
    InvalidGitSha {
        /// The rejected SHA string.
        value: String,
        /// Description of the validation failure.
        reason: String,
    },

    /// A schema version is outside the accepted range.
    #[error("unsupported schema version {value}; current maximum is {max}")]
    UnsupportedSchemaVersion {
        /// The rejected version number.
        value: u32,
        /// The highest version this build understands.
        max: u32,
    },

    /// A SHA-256 digest is not a valid 64-character hex string.
    #[error("invalid SHA-256 digest: {reason}")]
    InvalidSha256Digest {
        /// Description of the validation failure.
        reason: String,
    },
}

/// Result type alias using [`ArtefactError`].
pub type Result<T> = std::result::Result<T, ArtefactError>;
