//! The `check` port: readiness inspection, repair, and the Dylint run.
//!
//! These items form their own port surface because `check` needs stronger
//! guarantees than forwarding. It refuses to start Dylint until a readiness
//! check has approved the installation, and the approved value carries the
//! rule that governs a later re-check.
//!
//! Nothing here names the installer's own types. An adapter translates the
//! installer's failures into [`BootstrapFault`] as they cross this boundary,
//! so the command layer classifies and reports a defect in Whitaker's own
//! vocabulary rather than in the vocabulary of the crate that detected it.

use std::{ffi::OsString, io, io::Write};

use thiserror::Error;

use crate::{diagnostics::DiagnosticSettings, library_path::DirectLibraryPath};

/// Which record owns the library path a `check` run was approved with.
///
/// The route travels with the approved value because the run re-checks the
/// path, and the answer must be judged by the rule that approved it: an
/// installation's own directory that lost its suite is the installation's
/// defect and may be repaired, whereas the user's own path is theirs alone
/// and is never repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryRoute {
    /// The directory `whitaker install` recorded for this toolchain.
    Recorded,
    /// The path the user set in `DYLINT_LIBRARY_PATH`.
    UserManaged,
}

/// A validated installation a `check` run may lint with.
///
/// The value is produced only by a successful readiness check, so holding one
/// is the evidence that the installation was inspected before Dylint ran. The
/// bootstrap route carries the rule that approved its library path rather than
/// the approved run itself: the concrete run belongs to the adapter that
/// inspected the installation, and `check_run` takes it back from there.
///
/// The bootstrap variant also carries an [`ApprovalId`], which the
/// adapter increments for every readiness answer. `check_run` consumes the held
/// run only when the number matches the approval it holds, so a value produced
/// by an earlier preflight cannot be run against a later one's installation.
#[derive(Debug, Clone)]
pub enum CheckPrepared {
    /// The installed bootstrap is ready, and the adapter holds its prepared
    /// run for the `check_run` that follows.
    Bootstrap {
        /// Which record owns the approved library path.
        route: LibraryRoute,
        /// Which preflight produced this value.
        approval: ApprovalId,
    },
    /// The user's own library path holds the suite, so Dylint runs directly.
    Direct(DirectLibraryPath),
}

/// Which readiness answer produced a [`CheckPrepared`].
///
/// The adapter numbers every answer it gives, and the number travels with the
/// value so the run can prove it is consuming the approval that value names.
/// Without it, one adapter that answered twice would let the first value run
/// against the second answer's installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalId(u64);

impl ApprovalId {
    /// Name the approval a counter has reached.
    #[must_use]
    pub const fn new(number: u64) -> Self { Self(number) }

    /// The number this approval names.
    #[must_use]
    pub const fn number(self) -> u64 { self.0 }
}

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
    #[error(
        "Whitaker bootstrap metadata {path} is missing; run `whitaker install` to install the \
         lint suite and its Clippy snapshot"
    )]
    NotInstalled {
        /// The expected metadata file.
        path: String,
    },
    /// The recorded installation metadata cannot be used.
    #[error(
        "Whitaker bootstrap metadata {path} is unusable: {reason}; re-run `whitaker install` to \
         rewrite it"
    )]
    Metadata {
        /// The metadata file.
        path: String,
        /// Why it was rejected.
        reason: String,
    },
    /// The recorded lint-library directory no longer exists.
    #[error(
        "Whitaker lint libraries are missing from {path}; re-run `whitaker install` to stage them"
    )]
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
    #[error("could not query cargo-dylint: {reason}; re-run `whitaker install`")]
    CargoDylint {
        /// Why the query failed.
        reason: String,
    },
    /// The installed `cargo-dylint` is not the release the bootstrap was built
    /// for.
    #[error(
        "cargo-dylint reports version {found}, but this Whitaker installation was built for \
         {expected}; re-run `whitaker install` so the driver, snapshot, and tools agree"
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

/// Inspects, and runs Dylint against, the Whitaker installation.
///
/// This is the `check` port rather than a reuse of `LintRunner` because the
/// two commands need opposite guarantees. Forwarding runs whatever Dylint
/// finds and reports its status; `check` refuses to start Dylint until an
/// inspection has approved the installation, and returns the prepared value
/// that proves it.
pub trait CheckServices {
    /// Inspect the installation and approve it for the next run.
    ///
    /// The name states the mutation: a successful call does more than answer a
    /// question, because it holds a run that the next [`Self::check_run`] with
    /// the returned approval consumes. A second call supersedes the first, and
    /// the first value then reports [`CheckRunError::Unapproved`] rather than
    /// running against the second answer's installation.
    ///
    /// # Errors
    ///
    /// Returns an error naming what is wrong with the installation, or with
    /// the user's own library path when that path is authoritative.
    fn approve_readiness(&self) -> Result<CheckPrepared, CheckPreflightError>;

    /// Report whether the environment handed Dylint's libraries to the user.
    ///
    /// The command layer derives its repair policy from this answer, so the
    /// query is a fact about the environment rather than a decision: only the
    /// adapter reads the variable itself.
    fn library_path_is_user_managed(&self) -> bool;

    /// Provision the installation, then return once it should be ready.
    ///
    /// The caller holds no lock and passes no policy: the adapter re-checks the
    /// installation under the installation lock, so two concurrent checks
    /// install once rather than twice. `settings` is the root command's own
    /// diagnostic policy, so an implicit install is as quiet or as verbose as
    /// the invocation that asked for it.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRepairError`] when provisioning failed, when the lock
    /// could not be taken, or when the host failed before it could start.
    fn repair(
        &self,
        settings: DiagnosticSettings,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError>;

    /// Run Dylint against a prepared installation and report its status.
    ///
    /// The value names the route that approved it; the adapter that approved
    /// the installation holds its concrete run, and takes that run back here.
    /// No readiness check runs at this point, because the flow treats the
    /// re-check as its own step: repeating readiness would add a query the
    /// caller never asked for and could approve an installation the flow has
    /// not seen.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation stopped being usable, when the
    /// adapter holds no approved run, when the child cannot be started, or
    /// when Whitaker cannot write its own output.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError>;
}
