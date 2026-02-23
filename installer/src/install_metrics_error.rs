//! Error type for installer metrics persistence operations.

use std::path::PathBuf;

/// Errors that prevent metrics persistence.
#[derive(Debug, thiserror::Error)]
pub enum InstallMetricsError {
    /// Whitaker data directory could not be resolved.
    #[error("could not determine Whitaker data directory")]
    MissingDataDirectory,

    /// Creating the metrics directory failed.
    #[error("failed to create metrics directory {path}: {source}")]
    CreateDirectory {
        /// Directory path that could not be created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Reading the metrics file failed.
    #[error("failed to read metrics file {path}: {source}")]
    ReadMetrics {
        /// File path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Locking the metrics file failed.
    #[error("failed to lock metrics file {path}: {source}")]
    LockMetrics {
        /// File path that could not be locked.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Serializing metrics failed.
    #[error("failed to serialize metrics: {source}")]
    SerializeMetrics {
        /// Underlying serialization error.
        #[source]
        source: serde_json::Error,
    },

    /// Writing the metrics file failed.
    #[error("failed to write metrics file {path}: {source}")]
    WriteMetrics {
        /// File path that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}
