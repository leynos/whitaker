//! Error types for artefact packaging operations.
//!
//! Covers I/O failures, serialization problems, and validation errors
//! that can occur when creating `.tar.zst` archives and manifest files.

use thiserror::Error;

/// Errors arising from artefact packaging operations.
#[derive(Debug, Error)]
pub enum PackagingError {
    /// An I/O operation failed (reading source files, writing the archive).
    #[error("I/O error during packaging: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization of the manifest failed.
    #[error("manifest serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// No library files were provided for packaging.
    #[error("no library files provided for packaging")]
    EmptyFileList,
}
