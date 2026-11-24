//! Holding the concrete run a readiness check approved for the run that
//! follows.
//!
//! The port's prepared value names the rule that approved an installation, not
//! the installation itself, because the concrete `PreparedRun` belongs to the
//! installer and must stay inside its adapter. This module is where it stays:
//! readiness keeps the approved run here, and `check_run` takes it back.
//!
//! One adapter instance serves one invocation, and the flow asks readiness
//! immediately before the run that consumes its answer, so a single held value
//! is enough. Taking rather than reading it is what keeps "Dylint runs only
//! against a fresh approval" intact now that the port's value can be copied:
//! a second run of the same approval has no evidence left to run with.

use std::sync::{Mutex, MutexGuard, PoisonError};

use whitaker_installer::bootstrap::PreparedRun;

use crate::{
    library_path::DirectLibraryPath,
    ports::{CheckPrepared, CheckRunError, LibraryRoute},
};

/// The concrete run the most recent readiness check approved.
#[derive(Debug, Default)]
pub(crate) struct ApprovedRuns {
    /// The approved run, if readiness has approved one since the last run.
    ///
    /// The lock is held only to store or take the value, never across a child
    /// process, a write, or anything else that can fail, so a poisoned lock
    /// cannot mean a half-finished operation. Recovering the value is
    /// therefore sound, and refusing to run because of an unrelated panic
    /// would hide the failure that actually happened.
    held: Mutex<Option<PreparedRun>>,
}

impl ApprovedRuns {
    /// Start with no approved run.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            held: Mutex::new(None),
        }
    }

    /// Take the held approval, or report that the run began without one.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError::Unapproved`] when readiness has not approved
    /// an installation since the last run.
    pub(crate) fn take(&self) -> Result<PreparedRun, CheckRunError> {
        self.lock().take().ok_or(CheckRunError::Unapproved)
    }

    /// Keep `run` as the approval the next run consumes.
    fn hold(&self, run: PreparedRun) { *self.lock() = Some(run); }

    /// Drop any held approval, for a route that runs without one.
    fn clear(&self) { *self.lock() = None; }

    /// Lock the slot, recovering a poison as the type's documentation allows.
    fn lock(&self) -> MutexGuard<'_, Option<PreparedRun>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What a successful readiness check approved, and the evidence it approved.
///
/// Readiness produces this and the adapter's own `preflight` turns it into the
/// port's prepared value, so the concrete run is held by exactly the adapter
/// that inspected the installation.
#[derive(Debug)]
pub(crate) enum Approval {
    /// The installed bootstrap, with the run it prepared.
    Bootstrap {
        /// The bootstrap's approved run, including its library path.
        run: PreparedRun,
        /// Which record owns that run's library path.
        route: LibraryRoute,
    },
    /// The user's own library path holds the suite, so nothing was prepared.
    Direct(DirectLibraryPath),
}

impl Approval {
    /// Keep the concrete run, and return the port's own prepared value.
    ///
    /// A direct approval drops any run an earlier answer left behind, so the
    /// held value always describes the most recent approval rather than one
    /// the flow has since abandoned.
    #[must_use]
    pub(crate) fn record(self, held: &ApprovedRuns) -> CheckPrepared {
        match self {
            Self::Bootstrap { run, route } => {
                held.hold(run);
                CheckPrepared::Bootstrap { route }
            }
            Self::Direct(path) => {
                held.clear();
                CheckPrepared::Direct(path)
            }
        }
    }
}
