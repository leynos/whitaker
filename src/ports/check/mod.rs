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

use std::{ffi::OsString, io::Write};

mod errors;

pub use errors::{BootstrapFault, CheckPreflightError, CheckRepairError, CheckRunError};

use super::{direct_path::DirectLibraryPath, verbosity::RepairVerbosity};

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
/// Both variants carry an [`ApprovalId`], which the adapter increments for
/// every readiness answer. `check_run` consumes the approval only when the
/// number matches the one it holds, so a value produced by an earlier preflight
/// cannot run against a later one's answer, on either route.
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
    Direct {
        /// The user's own, already-validated library path.
        path: DirectLibraryPath,
        /// Which preflight produced this value.
        approval: ApprovalId,
    },
}

impl CheckPrepared {
    /// Prepare a direct run for a path that was validated by its constructor.
    ///
    /// A direct value names no held run, so the approval it carries says which
    /// answer superseded the last one. Callers outside the adapter use this
    /// constructor rather than building the variant, so a later field cannot
    /// be forgotten at a call site.
    #[must_use]
    pub const fn direct(path: DirectLibraryPath) -> Self {
        Self::Direct {
            path,
            approval: ApprovalId::new(0),
        }
    }
}

/// Which readiness answer produced a [`CheckPrepared`].
///
/// The adapter numbers every answer it gives, and the number travels with the
/// value so the run can prove it is consuming the approval that value names.
/// Without it, one adapter that answered twice would let the first value run
/// against the second answer's installation.
///
/// The number alone is not enough, because two adapter instances both start
/// counting at one: a value one instance issued would then name the other's
/// first run. The identifier therefore carries the issuer as well, and a store
/// refuses any identifier it did not issue. [`ApprovalId::new`] names no
/// issuer, so a hand-built value can never match a real store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalId {
    /// Which store issued this approval.
    issuer: u64,
    /// Which of that store's answers it names.
    number: u64,
}

impl ApprovalId {
    /// Name an approval no store issued.
    ///
    /// Used by [`CheckPrepared::direct`] so a caller can build a value that
    /// documents the direct route without being able to run with it. Issuers
    /// start at one, so the zero here never matches.
    #[must_use]
    pub const fn new(number: u64) -> Self { Self { issuer: 0, number } }

    /// Name the `number`th answer of the store identified by `issuer`.
    #[must_use]
    pub const fn issued_by(issuer: u64, number: u64) -> Self { Self { issuer, number } }

    /// The store that issued this approval, or zero for a hand-built value.
    #[must_use]
    pub const fn issuer(self) -> u64 { self.issuer }

    /// The number this approval names.
    #[must_use]
    pub const fn number(self) -> u64 { self.number }
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
    /// question, because it records an approval that the next
    /// [`Self::check_run`] carrying that number consumes. It also spawns one
    /// process — the `cargo dylint --version` probe the bootstrap readiness
    /// check makes — so this is not a pure read, and a caller must not treat
    /// it as one. The record lives
    /// behind the adapter's own lock rather than in this signature, so the
    /// port stays object-safe and the shared bundle keeps working; the rule is
    /// a documented contract, and `ApprovalId` is what enforces it. A second
    /// call supersedes the first, and the first value then reports
    /// [`CheckRunError::Unapproved`] rather than running against the second
    /// answer's installation.
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
    /// adapter reads the variable itself. It reads the snapshot the adapter
    /// set took when it was built, so it spawns nothing, writes nothing and
    /// touches no filesystem.
    fn library_path_is_user_managed(&self) -> bool;

    /// Provision the installation, then return once it should be ready.
    ///
    /// The caller holds no lock and passes no policy: the adapter re-checks the
    /// installation under the installation lock, so two concurrent checks
    /// install once rather than twice. `verbosity` is the root command's own
    /// `-q`/`-v` answer, as a value, so an implicit install is as quiet or as
    /// verbose as the invocation that asked for it without the port naming the
    /// binary's diagnostic policy type. This is the one method that writes the
    /// installation: it takes the lock, installs under it, and spawns the
    /// installer.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRepairError`] when provisioning failed, when the lock
    /// could not be taken, or when the host failed before it could start.
    fn repair(
        &self,
        verbosity: RepairVerbosity,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError>;

    /// Run Dylint against a prepared installation and report its status.
    ///
    /// The value names the route that approved it; the adapter that approved
    /// the installation holds its concrete run, and takes that run back here.
    /// No readiness check runs at this point, because the flow treats the
    /// re-check as its own step: repeating readiness would add a query the
    /// caller never asked for and could approve an installation the flow has
    /// not seen. The approval is consumed before the child is named, so a
    /// refused value starts nothing; a successful call spawns Dylint.
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
