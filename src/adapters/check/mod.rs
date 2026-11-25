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

use approval::Approval;
use fault::fault_of;
use repair::{HostEnv, SystemSteps, lock_fault, repair_production, repair_request};
use run::{BootstrappedRun, run_bootstrapped, run_direct};
use suite::{presence, recorded_suite_present, suite_present};
use whitaker_installer::{
    bootstrap::{RunOptions, preflight_with_cargo, run_prepared},
    dirs::{BaseDirs, SystemBaseDirs},
    orchestration::with_install_lock,
};

use crate::{
    diagnostics::DiagnosticSettings,
    ports::{
        BootstrapFault,
        CheckPreflightError,
        CheckPrepared,
        CheckRepairError,
        CheckRunError,
        CheckServices,
        DirectLibraryPath,
        LibraryRoute,
    },
};

mod approval;

pub(crate) use approval::ApprovedRuns;
mod fault;
mod repair;
mod run;
mod suite;
pub mod telemetry;

/// Which library set a `check` run should load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryMode {
    /// Load the libraries the installation records.
    Recorded,
    /// Load whatever the user's own `DYLINT_LIBRARY_PATH` names.
    UserManaged(OsString),
}

impl CheckServices for super::SystemServices {
    /// Inspect the installation the environment selects.
    ///
    /// A bootstrap route keeps the run that was approved in `self.approved`,
    /// because the port's prepared value carries only the route. One adapter
    /// serves one invocation, and the flow asks readiness immediately before
    /// the run that consumes the answer, so one held run is enough.
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
            LibraryMode::UserManaged(path) => preflight_user_managed(dirs, &path, inherited_path),
            LibraryMode::Recorded => preflight_recorded(dirs, inherited_path),
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
        settings: DiagnosticSettings,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError> {
        let Some(dirs) = self.platform_dirs.as_ref() else {
            return Err(CheckRepairError::DataDirectory);
        };
        // The snapshot the adapter set took, as for the readiness above.
        let host = HostEnv {
            dirs,
            inherited_path: self.inherited_path.as_deref(),
        };
        let request = repair_request(settings, self.ci.as_deref());
        with_install_lock(dirs, || {
            // The installer reserves standard output for machine-readable
            // markers. `check` owns only diagnostics, so markers reach the
            // same stream rather than whatever the user pipes from `check`.
            repair_production(host, request, stderr, &mut SystemSteps)
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
            CheckPrepared::Direct(library_path) => run_direct(library_path.as_os_str(), args),
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

/// Inspect `path` and prepare it as the user's own Dylint library path.
///
/// Readiness and this function judge the same listing with the same rules, so
/// a path approved by one is approved by the other. It is public because the
/// value it returns can only be obtained from an inspection: a caller that
/// substitutes the `check` port stages an approved path through the adapter
/// rather than forging one.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when the path names no
/// suite, [`CheckPreflightError::UserPathAmbiguous`] when it names more than
/// one, [`CheckPreflightError::UserPathUnusable`] when Dylint could not search
/// the list, and a bootstrap I/O error when an entry cannot be read.
pub fn prepare_direct_library_path(
    path: OsString,
) -> Result<DirectLibraryPath, CheckPreflightError> {
    let classified = presence(&path)?;
    DirectLibraryPath::new(path, classified)
}

/// Ask the installed bootstrap whether it is ready.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// bootstrap.
///
/// # Errors
///
/// Returns [`BootstrapFault::NoDataDirectory`] when the platform data
/// directory cannot be determined, and otherwise whatever
/// [`preflight_recorded_with`] reports for this machine.
fn preflight_recorded(
    system_dirs: Option<&SystemBaseDirs>,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(CheckPreflightError::Bootstrap(
            BootstrapFault::NoDataDirectory,
        ));
    };
    preflight_recorded_with(dirs, None, inherited_path)
}

/// Ask the recorded installation whether it is ready, with an optional
/// library-path override.
///
/// The override is the user's `DYLINT_LIBRARY_PATH` when they set one: the
/// bootstrap still supplies the Clippy snapshot and the driver slot, so only
/// the libraries Dylint loads come from the user. Either way the approved
/// path must name the Whitaker suite: the bootstrap only establishes that a
/// directory exists, and `check` always selects `--lib=whitaker_suite`.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// bootstrap; readiness does not read the environment itself.
///
/// # Errors
///
/// Returns the bootstrap's own readiness defect when the installation is not
/// usable, [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when the override names no single
/// suite Dylint could load, and [`BootstrapFault::LibrariesMissing`],
/// [`CheckPreflightError::RecordedPathAmbiguous`] or
/// [`CheckPreflightError::RecordedPathUnusable`] when the recorded library
/// directory no longer names exactly one suite.
fn preflight_recorded_with(
    dirs: &dyn BaseDirs,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    preflight_recorded_with_cargo(dirs, OsStr::new("cargo"), library_path, inherited_path)
}

/// [`preflight_recorded_with`] with an explicit `cargo` program.
///
/// The readiness probe runs `cargo dylint --version`, so a test that needs a
/// genuine `PreparedRun` from a host whose installed release may differ
/// supplies its own program here rather than replacing the readiness answer.
///
/// `inherited_path` is this process's `PATH` as the adapter set snapshotted
/// it. It is forwarded so the bootstrap can extend the child's `PATH` on
/// Windows, and is deliberately not read here: a helper that reads the
/// environment cannot be tested with a value the test chose.
///
/// # Errors
///
/// As for [`preflight_recorded_with`].
fn preflight_recorded_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let options = RunOptions {
        library_path,
        inherited_path,
    };
    let run = preflight_with_cargo(dirs, cargo, options)
        .map_err(|error| CheckPreflightError::Bootstrap(fault_of(error)))?;
    // The approved path's ownership is decided here, once, and travels with
    // the approval: the run re-check has to ask the same question, and
    // re-deriving the answer from the environment could not.
    let route = library_path.map_or(LibraryRoute::Recorded, |_| LibraryRoute::UserManaged);
    suite_rule(route, run.library_dir())?;
    Ok(Approval::Bootstrap { run, route })
}

/// Apply the suite rule the approved library path's route calls for.
///
/// Readiness and the run re-check both call this, so the two steps cannot
/// drift: a path approved by one is approved by the other, and a defect found
/// later carries the same reason an earlier one would have.
///
/// # Errors
///
/// Returns the same defects as [`recorded_suite_present`] for a recorded
/// directory, and as [`suite_present`] for a user-managed path.
fn suite_rule(route: LibraryRoute, path: &OsStr) -> Result<(), CheckPreflightError> {
    match route {
        LibraryRoute::Recorded => recorded_suite_present(path),
        LibraryRoute::UserManaged => suite_present(path),
    }
}

/// Decide what a user-managed library path needs before Dylint can run.
///
/// A recorded installation still provides the snapshot and driver, so the
/// readiness check runs with the user's path as the override. Only when no
/// installation has been recorded does the user's path have to supply
/// everything, and then it is inspected for a Whitaker suite: without one,
/// Dylint would fail with a status that looks like a lint result.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// recorded installation's readiness check.
///
/// # Errors
///
/// Returns the recorded installation's own defect when its readiness check
/// finds one, [`BootstrapFault::NoDataDirectory`] when the platform data
/// directory cannot be determined, [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when no installation is recorded
/// and the path names no single suite Dylint could load, and a bootstrap I/O
/// error when a directory on the path cannot be read.
fn preflight_user_managed(
    system_dirs: Option<&SystemBaseDirs>,
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    preflight_user_managed_in(
        system_dirs.map(|dirs| dirs as &dyn BaseDirs),
        path,
        inherited_path,
    )
}

/// Decide the user-managed route with the platform directories as a value.
///
/// The directories are a parameter rather than a lookup so the undeterminable
/// case is a decision this function makes, and a test can drive it. Without a
/// data directory there is no way to tell whether an installation is recorded,
/// so the route reports the host fault both routes share instead of silently
/// running a path the record might contradict.
///
/// `inherited_path` travels with the directories because a recorded
/// installation's readiness check is what consumes it.
///
/// # Errors
///
/// Returns [`BootstrapFault::NoDataDirectory`] when `system_dirs` is absent,
/// the recorded installation's own defect when one is recorded, and
/// [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when no installation is recorded
/// and `path` names no single suite Dylint could load.
fn preflight_user_managed_in(
    system_dirs: Option<&dyn BaseDirs>,
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(BootstrapFault::NoDataDirectory.into());
    };
    match preflight_recorded_with(dirs, Some(path), inherited_path) {
        Ok(approval) => Ok(approval),
        // No record means the legacy route: the user's path is the whole
        // installation. Any other defect is the installation's to report.
        Err(CheckPreflightError::Bootstrap(BootstrapFault::NotInstalled { .. })) => {
            // The path is listed once, and the rules judge that one listing,
            // so the prepared value is evidence Dylint may run rather than a
            // second inspection that could disagree with the first.
            Ok(Approval::Direct(prepare_direct_library_path(
                path.to_owned(),
            )?))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
