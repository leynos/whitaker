//! Failures of the installed Dylint bootstrap, each with its remedy.

use crate::clippy_snapshot::ClippySnapshotError;
use thiserror::Error;

/// The installed bootstrap could not prepare or run `cargo dylint`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BootstrapError {
    /// The platform data directory could not be determined.
    #[error("could not determine the platform data directory for the Whitaker bootstrap")]
    NoDataDirectory,

    /// The installer has not recorded a bootstrap here.
    #[error(
        "Whitaker bootstrap metadata {path} is missing; run whitaker-installer to \
         install the lint suite and its Clippy snapshot"
    )]
    NotInstalled {
        /// The expected metadata file.
        path: String,
    },

    /// The recorded bootstrap metadata cannot be used.
    #[error(
        "Whitaker bootstrap metadata {path} is unusable: {reason}; re-run \
         whitaker-installer to rewrite it"
    )]
    Metadata {
        /// The metadata file.
        path: String,
        /// Why it was rejected.
        reason: String,
    },

    /// The recorded lint-library directory no longer exists.
    #[error(
        "Whitaker lint libraries are missing from {path}; re-run whitaker-installer \
         to stage them"
    )]
    LibrariesMissing {
        /// The resolved library directory.
        path: String,
    },

    /// The Clippy snapshot is missing or invalid.
    #[error(transparent)]
    Snapshot(#[from] ClippySnapshotError),

    /// `cargo dylint --version` could not be run or parsed.
    #[error("could not query cargo-dylint: {reason}; re-run whitaker-installer")]
    CargoDylint {
        /// Why the query failed.
        reason: String,
    },

    /// The installed `cargo-dylint` is not the release the bootstrap was built for.
    #[error(
        "cargo-dylint reports version {found}, but this Whitaker installation was \
         bootstrapped for {expected}; re-run whitaker-installer so the driver, \
         snapshot, and tools agree"
    )]
    DylintVersion {
        /// The version recorded at installation.
        expected: String,
        /// The version on `PATH`.
        found: String,
    },

    /// A filesystem operation failed.
    #[error("Whitaker bootstrap I/O failed for {path}: {reason}")]
    Io {
        /// The path being accessed.
        path: String,
        /// The underlying error.
        reason: String,
    },

    /// `cargo dylint` could not be started.
    #[error("could not start `cargo dylint`: {reason}")]
    Spawn {
        /// Why the process could not start.
        reason: String,
    },
}

impl BootstrapError {
    /// Build an [`Self::Io`] error for `path`.
    pub(crate) fn io(path: impl ToString, error: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_string(),
            reason: error.to_string(),
        }
    }
}
