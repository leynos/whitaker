//! The `check` port: readiness inspection, repair, and the Dylint run.
//!
//! These items form their own port surface because `check` needs stronger
//! guarantees than forwarding. It refuses to start Dylint until a readiness
//! check has approved the installation, and the approved value carries both
//! the evidence and the rule that governs a later re-check.

use std::{ffi::OsString, io, io::Write};

use thiserror::Error;
use whitaker_installer::bootstrap::{BootstrapError, PreparedRun};

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
/// is the evidence that the installation was inspected before Dylint ran.
#[derive(Debug, Clone)]
pub enum CheckPrepared {
    /// The installed bootstrap is ready, and knows where its libraries are.
    Bootstrap {
        /// The bootstrap's approved run, including its library path.
        run: PreparedRun,
        /// Which record owns that library path.
        route: LibraryRoute,
    },
    /// The user's own library path holds the suite, so Dylint runs directly.
    Direct(DirectLibraryPath),
}

/// Why a readiness check did not approve an installation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckPreflightError {
    /// The installed bootstrap reported what is wrong.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
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
    Bootstrap(#[from] BootstrapError),
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
    /// Inspect the installation and return the evidence that Dylint may run.
    ///
    /// # Errors
    ///
    /// Returns an error naming what is wrong with the installation, or with
    /// the user's own library path when that path is authoritative.
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError>;

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
    /// # Errors
    ///
    /// Returns an error when the installation stopped being usable, when the
    /// child cannot be started, or when Whitaker cannot write its own output.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError>;
}
