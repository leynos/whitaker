//! The error vocabulary the `check` port hands back across its boundary.
//!
//! An adapter translates the installer's own failures into these types as they
//! cross the boundary, so the command layer classifies and reports a defect in
//! Whitaker's vocabulary rather than in the vocabulary of the crate that
//! detected it. They live apart from the port surface because callers name
//! them far more often than they name the trait, and because each carries the
//! payloads and documentation that would otherwise crowd the port's own
//! definition.

use std::io;

use thiserror::Error;

/// Why the installed bootstrap could not prepare, or run, Dylint.
///
/// The variants are the distinctions the command layer classifies: an
/// installation that is missing or stale (provisioning can mend it) and a host
/// fault (nothing can). Each payload is the human-readable fact the adapter
/// learned from the installer, carried as data so the port needs no installer
/// type to describe it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BootstrapFault {
    /// The platform data directory could not be determined.
    #[error("could not determine the platform data directory for the Whitaker bootstrap")]
    NoDataDirectory,
    /// The installation record is missing.
    #[error("Whitaker bootstrap metadata {path} is missing")]
    NotInstalled {
        /// The expected metadata file.
        path: String,
    },
    /// The recorded installation metadata cannot be used.
    #[error("Whitaker bootstrap metadata {path} is unusable: {reason}")]
    Metadata {
        /// The metadata file.
        path: String,
        /// Why it was rejected.
        reason: String,
    },
    /// The recorded lint-library directory no longer exists.
    #[error("Whitaker lint libraries are missing from {path}")]
    LibrariesMissing {
        /// The resolved library directory.
        path: String,
    },
    /// The Clippy snapshot is missing or invalid.
    #[error("the recorded Clippy snapshot is missing or invalid: {reason}")]
    Snapshot {
        /// Why the snapshot was rejected.
        reason: String,
    },
    /// The Clippy snapshot exists but could not be read.
    ///
    /// Separate from [`Self::Snapshot`] because a denied or failed read is the
    /// host's fault: installing again cannot repair permissions or a disk.
    #[error("the Clippy snapshot at {path} could not be read: {reason}")]
    SnapshotIo {
        /// The path being accessed.
        path: String,
        /// The underlying error.
        reason: String,
    },
    /// `cargo dylint --version` could not be run or parsed.
    #[error("could not query cargo-dylint: {reason}")]
    CargoDylint {
        /// Why the query failed.
        reason: String,
    },
    /// The installed `cargo-dylint` is not the release the bootstrap was built
    /// for.
    #[error(
        "cargo-dylint reports version {found}, but this Whitaker installation was built for \
         {expected}"
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

/// Why a readiness check did not approve an installation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckPreflightError {
    /// The installed bootstrap reported what is wrong.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapFault),
    /// The user managed the library path, and it names no Whitaker suite.
    ///
    /// Only the user can repair this: installing Whitaker elsewhere cannot
    /// change a directory the user chose.
    #[error("the Dylint library path names no Whitaker suite library")]
    UserPathHasNoSuite,
    /// The user managed the library path, and it names more than one suite.
    ///
    /// Dylint refuses to choose between them and exits with the same status a
    /// lint denial uses, so the ambiguity is refused before Dylint starts.
    #[error("the Dylint library path names more than one Whitaker suite library")]
    UserPathAmbiguous,
    /// The user managed the library path, and Dylint could not search it.
    ///
    /// Dylint requires every list entry to be an absolute path naming an
    /// existing directory, and rejects the whole list otherwise. Its refusal
    /// uses the status a lint denial uses, so the list is refused here first.
    #[error("the Dylint library path entry `{path}` {reason}")]
    UserPathUnusable {
        /// The entry that cannot be searched.
        path: String,
        /// What is wrong with the entry.
        reason: &'static str,
    },
    /// The installation's recorded library directory names more than one
    /// suite.
    ///
    /// Dylint refuses to choose between them and exits with the status a lint
    /// denial uses, and the run would pass that status through as a lint
    /// result. Unlike [`Self::UserPathAmbiguous`], this directory came from
    /// the installation's own record, so provisioning is the honest advice.
    #[error(
        "the recorded lint-library directory `{path}` names more than one Whitaker suite library"
    )]
    RecordedPathAmbiguous {
        /// The recorded directory that holds several suites.
        path: String,
    },
    /// The installation's recorded library path is not one Dylint can search.
    ///
    /// Dylint requires every list entry to be an absolute path naming an
    /// existing directory. The record holds one directory, so a record that
    /// fails this is a provisioning defect rather than a lint result.
    #[error("the recorded lint-library path entry `{path}` {reason}")]
    RecordedPathUnusable {
        /// The recorded entry that cannot be searched.
        path: String,
        /// What is wrong with the entry.
        reason: &'static str,
    },
}

/// Why a repair attempt did not complete.
///
/// The variants separate what a person can act on: an installer that ran and
/// failed, a lock that another process held, and a host that could not offer
/// the lock at all. Folding them together would turn "wait for the other
/// install" into "reinstall Whitaker".
#[derive(Debug, Error)]
pub enum CheckRepairError {
    /// The installer ran and did not complete.
    #[error("installing Whitaker did not complete: {message}")]
    Install {
        /// Human-readable installer details.
        message: String,
    },
    /// The installation lock could not be taken, so nothing was installed.
    #[error("could not take the Whitaker installation lock: {message}")]
    Lock {
        /// Human-readable lock error details.
        message: String,
    },
    /// The platform data directory could not be determined.
    #[error("the platform data directory could not be determined")]
    DataDirectory,
    /// Whitaker could not write its own output before provisioning started.
    #[error("could not write command output")]
    Output {
        /// The operating-system write error.
        #[source]
        source: io::Error,
    },
    /// The host failed before provisioning could start.
    #[error("the host failed before provisioning: {message}")]
    Host {
        /// Human-readable host error details.
        message: String,
    },
}

impl CheckRepairError {
    /// Return the bounded category this failure belongs to.
    ///
    /// A log line carries the category rather than the error's own text: the
    /// payload may name a path the user chose, and a diagnostic stream is not
    /// the place for it. The typed error still reaches the caller, which
    /// classifies it without rendering the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::CheckRepairError;
    ///
    /// assert_eq!(CheckRepairError::DataDirectory.category(), "data-directory");
    /// ```
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::Install { .. } => "install",
            Self::Lock { .. } => "lock",
            Self::DataDirectory => "data-directory",
            Self::Output { .. } => "output",
            Self::Host { .. } => "host",
        }
    }
}

/// Why a lint run could not report a status.
#[derive(Debug, Error)]
pub enum CheckRunError {
    /// The installation stopped naming exactly one suite before Dylint could
    /// start.
    ///
    /// Readiness approved the directory moments earlier, so this is a change
    /// the run must report as provisioning, not a lint result.
    #[error(transparent)]
    Preflight(#[from] CheckPreflightError),
    /// The bootstrap failed once the run had been approved.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapFault),
    /// The adapter held no approved run when the run began.
    ///
    /// The port's prepared value names only the route, so the concrete run is
    /// kept by the adapter that approved it and taken back here. One adapter
    /// serves one invocation and readiness is asked immediately before the run
    /// that consumes its answer, so a missing value is a defect in the caller
    /// rather than in the installation: it is reported rather than repaired.
    #[error("the approved Whitaker installation was not held when the run began")]
    Unapproved,
    /// `cargo dylint` could not be started on the direct path.
    #[error("could not start cargo dylint: {source}")]
    Spawn {
        /// The operating-system process error.
        #[source]
        source: io::Error,
    },
    /// Whitaker could not write its own output.
    #[error("could not write command output")]
    Output {
        /// The operating-system write error.
        #[source]
        source: io::Error,
    },
}

impl CheckRunError {
    /// Return the bounded category this failure belongs to.
    ///
    /// A run failure can name a suite directory the user chose, so a log line
    /// carries this category instead of the error's text. The typed error
    /// still reaches the caller for classification.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::ports::CheckRunError;
    ///
    /// assert_eq!(CheckRunError::Unapproved.category(), "unapproved");
    /// ```
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::Preflight(_) => "preflight",
            Self::Bootstrap(_) => "bootstrap",
            Self::Unapproved => "unapproved",
            Self::Spawn { .. } => "spawn",
            Self::Output { .. } => "output",
        }
    }
}
