//! Failures of the Clippy snapshot boundary, each with its remedy.
//!
//! Every variant names the path or asset involved and states what to do next.
//! None of them is recoverable by cloning public Clippy: Whitaker owns the
//! history the Dylint driver build reads, so a missing or invalid snapshot is
//! a broken installation to repair, never a cue to fall back to the network.

use thiserror::Error;

/// Appended to every failure so nobody mistakes one for a transient fetch.
const NO_FALLBACK: &str = "Whitaker never falls back to cloning public Clippy";

/// A Clippy snapshot could not be built, installed, or trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClippySnapshotError {
    /// The embedded or supplied snapshot manifest is malformed.
    #[error("invalid Clippy snapshot manifest: {reason}")]
    Manifest {
        /// Why the manifest was rejected.
        reason: String,
    },

    /// The snapshot bundle file does not exist.
    #[error(
        "Clippy snapshot bundle {path} is missing; download \
         {asset} from the Whitaker release or re-run whitaker-installer. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    BundleMissing {
        /// Where the bundle was expected.
        path: String,
        /// The release asset that provides it.
        asset: String,
    },

    /// The `.sha256` sidecar is absent or does not contain a digest.
    #[error(
        "Clippy snapshot checksum sidecar {path} is unusable: {reason}; \
         download it again beside the bundle. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    Sidecar {
        /// The sidecar path.
        path: String,
        /// Why it was rejected.
        reason: String,
    },

    /// The bundle's bytes do not match its published digest.
    #[error(
        "Clippy snapshot bundle {path} failed SHA-256 verification: expected \
         {expected}, got {actual}; the download is corrupt or was tampered with. \
         Delete it and download it again. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    ChecksumMismatch {
        /// The bundle path.
        path: String,
        /// The digest the sidecar publishes.
        expected: String,
        /// The digest of the bytes on disk.
        actual: String,
    },

    /// The release does not publish the snapshot for this commit.
    #[error(
        "Clippy snapshot asset {url} was not found; this Whitaker release \
         does not publish the snapshot it pins. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    AssetNotFound {
        /// The asset URL that returned 404.
        url: String,
    },

    /// Downloading the snapshot failed.
    #[error(
        "could not download Clippy snapshot {asset}: {reason}; check access to \
         the Whitaker release and retry. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    Download {
        /// The asset name.
        asset: String,
        /// Why the download failed.
        reason: String,
    },

    /// No installed snapshot generation exists where one is required.
    #[error(
        "Clippy snapshot {path} is not installed; run whitaker-installer to \
         install it. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    NotInstalled {
        /// The expected generation directory.
        path: String,
    },

    /// A repository failed one of the snapshot integrity checks.
    #[error(
        "Clippy snapshot {path} is invalid: {check}; remove {path} and re-run \
         the installation. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    Invalid {
        /// The repository that failed verification.
        path: String,
        /// The failed check, phrased as what is wrong.
        check: String,
    },

    /// The history does not reach the revision Dylint must select.
    #[error(
        "Clippy snapshot {path} has insufficient history for {toolchain}: \
         {reason}; publish a snapshot whose first-parent history reaches the \
         revision that introduced {toolchain}. {NO_FALLBACK}",
        NO_FALLBACK = NO_FALLBACK
    )]
    InsufficientHistory {
        /// The repository that was searched.
        path: String,
        /// The pinned nightly.
        toolchain: String,
        /// What the search found instead.
        reason: String,
    },

    /// A Git subprocess failed.
    #[error("git {operation} failed for the Clippy snapshot: {message}")]
    Git {
        /// The Git operation.
        operation: &'static str,
        /// Git's diagnostic.
        message: String,
    },

    /// A filesystem operation on the snapshot root failed.
    #[error("Clippy snapshot I/O failed for {path}: {reason}")]
    Io {
        /// The path being accessed.
        path: String,
        /// The underlying error.
        reason: String,
    },
}

impl ClippySnapshotError {
    /// Build an [`Self::Invalid`] error for `path`.
    #[must_use]
    pub fn invalid(path: impl ToString, check: impl Into<String>) -> Self {
        Self::Invalid {
            path: path.to_string(),
            check: check.into(),
        }
    }

    /// Build an [`Self::Io`] error for `path`.
    #[must_use]
    pub fn io(path: impl ToString, error: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_string(),
            reason: error.to_string(),
        }
    }
}
