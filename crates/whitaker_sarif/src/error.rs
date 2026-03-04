//! Error types for SARIF model and merge operations.
//!
//! This module defines [`SarifError`], the crate-level error enum, and a
//! convenience [`Result`] type alias. All fallible operations in the crate
//! return `Result<T, SarifError>`.

use thiserror::Error;

/// Errors arising from SARIF serialization, deserialization, I/O, and merge
/// operations.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::SarifError;
///
/// let err = SarifError::InvalidLevel("unknown".into());
/// assert!(err.to_string().contains("unknown"));
/// ```
#[derive(Debug, Error)]
pub enum SarifError {
    /// JSON serialization or deserialization failed.
    #[error("SARIF serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// File I/O operation failed.
    #[error("SARIF I/O operation failed: {0}")]
    Io(#[from] std::io::Error),

    /// An invalid SARIF level string was provided.
    #[error("invalid SARIF level: {0}")]
    InvalidLevel(String),

    /// A merge conflict occurred.
    #[error("merge conflict: {0}")]
    MergeConflict(String),

    /// A required builder field was not set.
    #[error("missing required field: {0}")]
    MissingField(String),
}

/// Convenience alias for results using [`SarifError`].
pub type Result<T> = std::result::Result<T, SarifError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_level_formats_message() {
        let err = SarifError::InvalidLevel("bad".into());
        assert_eq!(err.to_string(), "invalid SARIF level: bad");
    }

    #[test]
    fn merge_conflict_formats_message() {
        let err = SarifError::MergeConflict("empty runs".into());
        assert_eq!(err.to_string(), "merge conflict: empty runs");
    }

    #[test]
    fn missing_field_formats_message() {
        let err = SarifError::MissingField("rule_id".into());
        assert_eq!(err.to_string(), "missing required field: rule_id");
    }
}
