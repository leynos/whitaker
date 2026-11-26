//! The `check` adapter: readiness inspection, and the Dylint run behind it.
//!
//! The adapter has two routes, and which one applies is decided by the
//! environment rather than by the caller. A machine that manages its own
//! `DYLINT_LIBRARY_PATH` has already said which libraries to load, so Whitaker
//! inspects that path and runs `cargo dylint` directly; on any other machine
//! the installed bootstrap supplies the libraries, the Clippy snapshot and the
//! managed driver, and is asked whether it is ready.
//!
//! The snapshot [`SystemServices`](crate::adapters::SystemServices) takes when
//! it is built supplies every environment fact this adapter needs:
//! `DYLINT_LIBRARY_PATH` selects the route, `CI` adds the repair advice, and
//! `PATH` travels down as a value because the bootstrap extends the child's
//! `PATH` with it on Windows. Every question one command asks therefore sees
//! the same environment. Nothing below this module reads the process
//! environment: the command layer is handed values.
//!
//! The installer's own types stop at this module. Readiness produces a
//! `PreparedRun` and the failures are `BootstrapError`s, so the approved run is
//! kept by the private approval holder and every failure is translated by the
//! fault translator into the port's vocabulary before it leaves the adapter.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

use preflight::{preflight_recorded, preflight_user_managed};
use repair::{HostEnv, RepairStreams, SystemSteps, lock_fault, repair_production, repair_request};
use run::{BootstrappedRun, run_bootstrapped, run_direct_with_cargo};
use whitaker_installer::{bootstrap::run_prepared, orchestration::with_install_lock};

use crate::ports::{
    CheckPreflightError,
    CheckPrepared,
    CheckRepairError,
    CheckRunError,
    CheckServices,
    RepairVerbosity,
};

mod approval;

pub(crate) use approval::ApprovedRuns;
mod fault;
mod preflight;
mod repair;
mod run;
mod suite;
pub mod telemetry;

pub use preflight::prepare_direct_library_path;
pub(super) use preflight::{preflight_recorded_with, suite_rule};
#[cfg(all(test, unix))]
pub(super) use {approval::Approval, preflight::preflight_recorded_with_cargo};

/// Which library set a `check` run should load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryMode {
    /// Load the libraries the installation records.
    Recorded,
    /// Load whatever the user's own `DYLINT_LIBRARY_PATH` names.
    UserManaged(OsString),
}

impl CheckServices for super::SystemServices {
    /// Inspect the installation and approve it for one run.
    ///
    /// This is the port's one read-then-write method: it inspects the
    /// installation, then advances the approval store so the value it returns
    /// is single-use. The write is what makes the method a command rather than
    /// a query, which is why the flow asks it again immediately before each
    /// run instead of caching the first answer.
    ///
    /// A bootstrap route keeps the concrete run in `self.approved`, because
    /// the port's prepared value carries only the route and the approval
    /// number. One adapter serves one invocation, so one held run is enough.
    ///
    /// # Errors
    ///
    /// Returns [`CheckPreflightError::Bootstrap`] with the typed reason when
    /// the installation is not ready,
    /// [`CheckPreflightError::UserPathHasNoSuite`],
    /// [`CheckPreflightError::UserPathAmbiguous`],
    /// [`CheckPreflightError::UserPathUnusable`],
    /// [`CheckPreflightError::RecordedPathAmbiguous`] or
    /// [`CheckPreflightError::RecordedPathUnusable`] when the library path
    /// does not name exactly one Whitaker suite, and a bootstrap I/O error
    /// when a directory on that path cannot be read.
    fn approve_readiness(&self) -> Result<CheckPrepared, CheckPreflightError> {
        // The adapter set snapshotted `PATH`; the bootstrap only needs it to
        // extend the child's `PATH` on Windows.
        let inherited_path = self.inherited_path.as_deref();
        let dirs = self.platform_dirs.as_ref();
        let approval = match mode_for(self.dylint_library_path.as_deref()) {
            LibraryMode::UserManaged(path) => {
                preflight_user_managed(dirs, &self.cargo, &path, inherited_path)
            }
            LibraryMode::Recorded => preflight_recorded(dirs, &self.cargo, inherited_path),
        }?;
        Ok(approval.record(&self.approved))
    }

    /// Report whether `DYLINT_LIBRARY_PATH` names libraries the user manages.
    fn library_path_is_user_managed(&self) -> bool {
        !matches!(
            mode_for(self.dylint_library_path.as_deref()),
            LibraryMode::Recorded
        )
    }

    /// Provision the installation, re-checking it under the lock first.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRepairError`] when the lock cannot be taken, when the
    /// installer fails, or when the host cannot offer a lock at all.
    fn repair(
        &self,
        verbosity: RepairVerbosity,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError> {
        let Some(dirs) = self.platform_dirs.as_ref() else {
            return Err(CheckRepairError::DataDirectory);
        };
        // The snapshot the adapter set took, as for the readiness above.
        let host = HostEnv {
            dirs,
            inherited_path: self.inherited_path.as_deref(),
            cargo: &self.cargo,
        };
        let request = repair_request(verbosity, self.ci.as_deref());
        with_install_lock(dirs, || {
            // The installer reserves standard output for machine-readable
            // markers. `check` owns only diagnostics, so markers reach the
            // same stream rather than whatever the user pipes from `check`.
            // The installer's markers take a stream of their own rather than
            // the caller's diagnostics: `check` owns only diagnostics, so the
            // markers must not reach whatever the user reads.
            let mut markers = repair::marker_stream();
            repair_production(
                host,
                request,
                RepairStreams {
                    installer: &mut markers,
                    diagnostics: stderr,
                },
                &mut SystemSteps,
            )
        })
        .map_err(|error| lock_fault(&error))?
    }

    /// Run Dylint, printing the built-driver notice when one was built.
    ///
    /// The bootstrap route takes the run readiness kept under the approval the
    /// value names, which is why the port value names only the route and that
    /// approval: Dylint never runs against an installation this adapter did
    /// not just approve, a second run of the same value has nothing left to run
    /// with, and a value from an earlier preflight cannot consume a later
    /// one's run.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError`] when the installation stopped being usable,
    /// when no approved run is held, or when the child cannot be started. A
    /// notice that cannot be written is logged and does not fail the call: the
    /// run it describes already happened.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        match prepared {
            CheckPrepared::Bootstrap { route, approval } => {
                let run = self.approved.take(approval)?;
                run_bootstrapped(
                    BootstrappedRun {
                        prepared: &run,
                        route,
                        args,
                    },
                    stderr,
                    run_prepared,
                )
            }
            CheckPrepared::Direct { path, approval } => {
                // The approval is spent before the child is named, so a
                // refused value cannot start Dylint even once.
                self.approved.consume(approval)?;
                run_direct_with_cargo(path.as_os_str(), args, &self.cargo)
            }
        }
    }
}

/// Choose the library mode for the value `DYLINT_LIBRARY_PATH` held.
///
/// An empty value is what a shell leaves behind for `VAR=`, and treating it as
/// authoritative would suppress the recorded installation for no reason.
fn mode_for(explicit: Option<&OsStr>) -> LibraryMode {
    explicit
        .filter(|path| !path.is_empty())
        .map_or(LibraryMode::Recorded, |path| {
            LibraryMode::UserManaged(path.to_owned())
        })
}

#[cfg(test)]
mod tests;
