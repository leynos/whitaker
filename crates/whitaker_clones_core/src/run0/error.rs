//! Typed errors for token-pass acceptance and SARIF emission.

use thiserror::Error;

/// Result type for Run 0 acceptance and emission operations.
pub type Run0Result<T> = Result<T, Run0Error>;

/// Errors raised while accepting candidate pairs or emitting SARIF Run 0.
#[derive(Debug, Error)]
pub enum Run0Error {
    /// A candidate or accepted pair referenced an unknown fragment ID.
    #[error("missing token fragment `{fragment_id}`")]
    MissingFragment {
        /// Fragment identifier that was referenced but not supplied.
        fragment_id: String,
    },

    /// A fragment had no retained fingerprints, so set-based scoring was not possible.
    #[error("token fragment `{fragment_id}` must retain at least one fingerprint")]
    EmptyFingerprintSet {
        /// Fragment identifier with an empty retained set.
        fragment_id: String,
    },

    /// The pair resolved to fragments emitted under different normalization profiles.
    #[error(
        "candidate pair `{left_fragment}` and `{right_fragment}` must share the same normalization profile"
    )]
    MixedProfiles {
        /// Left fragment identifier.
        left_fragment: String,
        /// Right fragment identifier.
        right_fragment: String,
    },

    /// A configured threshold was malformed.
    #[error("similarity threshold `{name}` must satisfy 0 < numerator <= denominator")]
    InvalidThreshold {
        /// Human-readable threshold name.
        name: String,
    },

    /// A retained fingerprint byte range could not be mapped back to source text.
    #[error(
        "fingerprint range {start}..{end} for `{fragment_id}` is invalid for source length {source_len}"
    )]
    InvalidFingerprintRange {
        /// Fragment identifier.
        fragment_id: String,
        /// Range start.
        start: usize,
        /// Range end.
        end: usize,
        /// Source length in bytes.
        source_len: usize,
    },

    /// The source text was not valid UTF-8 at the reported byte boundary.
    #[error("fingerprint range for `{fragment_id}` does not align to UTF-8 character boundaries")]
    InvalidUtf8Boundary {
        /// Fragment identifier.
        fragment_id: String,
    },

    /// SARIF model construction failed.
    #[error(transparent)]
    Sarif(#[from] whitaker_sarif::SarifError),

    /// Internal decimal parsing for Whitaker properties failed unexpectedly.
    #[error("failed to convert similarity ratio `{value}` into a finite SARIF score")]
    InvalidScore {
        /// The decimal string that failed to parse.
        value: String,
    },
}
