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

use crate::ports::{ApprovalId, CheckPrepared, CheckRunError, DirectLibraryPath, LibraryRoute};

/// The concrete run the most recent readiness check approved.
#[derive(Debug, Default)]
pub(crate) struct ApprovedRuns {
    /// The next approval number, and the run the current one covers.
    ///
    /// The lock is held only to store or take the value, never across a child
    /// process, a write, or anything else that can fail, so a poisoned lock
    /// cannot mean a half-finished operation. Recovering the value is
    /// therefore sound, and refusing to run because of an unrelated panic
    /// would hide the failure that actually happened.
    held: Mutex<Held>,
}

/// One approval number and the run it covers.
#[derive(Debug, Default)]
struct Held {
    /// The last number handed out, which starts at zero before any answer.
    issued: u64,
    /// The number that is still unspent, if any.
    ///
    /// A number becomes spendable when it is issued and stops being spendable
    /// once it is consumed, so a number that was never issued and a number
    /// already spent are refused the same way. The slot is deliberately left
    /// alone on a mismatch: refusing a stale value must not invalidate the
    /// approval that is still current.
    active: Option<u64>,
    /// The run the active number covers, if it prepared one.
    run: Option<PreparedRun>,
}

impl ApprovedRuns {
    /// Start with no approved run.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            held: Mutex::new(Held {
                issued: 0,
                active: None,
                run: None,
            }),
        }
    }

    /// Take the bootstrap run `approval` names, or report that none is held.
    ///
    /// The number is compared before anything is taken, so a stale value is
    /// refused without consuming the run its own approval still covers. Both
    /// steps happen under one lock, so no answer can replace the run between
    /// the comparison and the take.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError::Unapproved`] when no run is held, or when the
    /// held run belongs to a different approval: a value produced by one
    /// preflight must not run against another's installation.
    pub(crate) fn take(&self, approval: ApprovalId) -> Result<PreparedRun, CheckRunError> {
        let mut held = self.lock();
        if held.active != Some(approval.number()) {
            return Err(CheckRunError::Unapproved);
        }
        let run = held.run.take();
        held.active = None;
        run.ok_or(CheckRunError::Unapproved)
    }

    /// Consume `approval` if it is the current answer, whatever its route.
    ///
    /// A direct route holds no run, but it still supersedes an earlier answer
    /// and can itself be superseded, so the run that follows must prove it is
    /// consuming the newest one.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError::Unapproved`] when `approval` is not the newest
    /// answer, which is the same refusal a stale bootstrap value receives.
    pub(crate) fn consume(&self, approval: ApprovalId) -> Result<(), CheckRunError> {
        let mut held = self.lock();
        if held.active != Some(approval.number()) {
            return Err(CheckRunError::Unapproved);
        }
        held.run = None;
        held.active = None;
        Ok(())
    }

    /// Issue the next approval number and keep `run` under it.
    fn hold(&self, run: PreparedRun) -> ApprovalId {
        let mut held = self.lock();
        held.issued += 1;
        held.active = Some(held.issued);
        held.run = Some(run);
        ApprovalId::new(held.issued)
    }

    /// Issue the next approval number without holding a run.
    fn issue(&self) -> ApprovalId {
        let mut held = self.lock();
        held.issued += 1;
        held.active = Some(held.issued);
        held.run = None;
        ApprovalId::new(held.issued)
    }

    /// Lock the slot, recovering a poison as the type's documentation allows.
    fn lock(&self) -> MutexGuard<'_, Held> {
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
    /// Each answer takes the next approval number, so the value names the
    /// answer that produced it. A direct approval holds no run and drops any
    /// earlier one, so the held slot always describes the most recent answer
    /// rather than one the flow has since abandoned.
    #[must_use]
    pub(crate) fn record(self, held: &ApprovedRuns) -> CheckPrepared {
        match self {
            Self::Bootstrap { run, route } => CheckPrepared::Bootstrap {
                route,
                approval: held.hold(run),
            },
            Self::Direct(path) => CheckPrepared::Direct {
                path,
                approval: held.issue(),
            },
        }
    }
}
