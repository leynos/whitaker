//! The `check` adapter: readiness inspection, and the Dylint run behind it.
//!
//! The adapter has two routes, and which one applies is decided by the
//! environment rather than by the caller. A machine that manages its own
//! `DYLINT_LIBRARY_PATH` has already said which libraries to load, so Whitaker
//! inspects that path and runs `cargo dylint` directly; on any other machine
//! the installed bootstrap supplies the libraries, the Clippy snapshot and the
//! managed driver, and is asked whether it is ready.
//!
//! Only this module reads those environment variables, once per invocation,
//! at the production edge: `DYLINT_LIBRARY_PATH` selects the route, and
//! `PATH` travels down as a value because the bootstrap extends the child's
//! `PATH` with it on Windows. Nothing below this module reads the process
//! environment: the command layer is handed values.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

use repair::{HostEnv, SystemSteps, lock_fault, repair_production, repair_request};
use run::{BootstrappedRun, run_bootstrapped, run_direct};
use suite::{recorded_suite_present, suite_present};
use whitaker_installer::{
    bootstrap::{BootstrapError, RunOptions, preflight_with_cargo, run_prepared},
    dirs::{BaseDirs, SystemBaseDirs},
    orchestration::with_install_lock,
};

use crate::{
    diagnostics::DiagnosticSettings,
    ports::{
        CheckPreflightError,
        CheckPrepared,
        CheckRepairError,
        CheckRunError,
        CheckServices,
        DirectLibraryPath,
        LibraryRoute,
    },
};

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
    /// # Errors
    ///
    /// Returns [`CheckPreflightError::Bootstrap`] with the typed reason when
    /// the installation is not ready,
    /// [`CheckPreflightError::UserPathHasNoSuite`],
    /// [`CheckPreflightError::UserPathAmbiguous`] or
    /// [`CheckPreflightError::RecordedPathAmbiguous`] when the library path
    /// does not name exactly one Whitaker suite, and a bootstrap I/O error
    /// when a directory on that path cannot be read.
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError> {
        // The composition root reads `PATH` once and hands it down as a value;
        // the bootstrap only needs it to extend the child's `PATH` on Windows.
        let inherited_path = std::env::var_os("PATH");
        match library_mode() {
            LibraryMode::UserManaged(path) => {
                preflight_user_managed(&path, inherited_path.as_deref())
            }
            LibraryMode::Recorded => preflight_recorded(inherited_path.as_deref()),
        }
    }

    /// Report whether `DYLINT_LIBRARY_PATH` names libraries the user manages.
    fn library_path_is_user_managed(&self) -> bool {
        !matches!(library_mode(), LibraryMode::Recorded)
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
        let Some(dirs) = SystemBaseDirs::new() else {
            return Err(CheckRepairError::DataDirectory);
        };
        // The composition root reads `PATH` once, as for the readiness above.
        let inherited_path = std::env::var_os("PATH");
        let host = HostEnv {
            dirs: &dirs,
            inherited_path: inherited_path.as_deref(),
        };
        let request = repair_request(settings, std::env::var_os("CI").as_deref());
        with_install_lock(&dirs, || {
            // The installer reserves standard output for machine-readable
            // markers. `check` owns only diagnostics, so markers reach the
            // same stream rather than whatever the user pipes from `check`.
            repair_production(host, request, stderr, &mut SystemSteps)
        })
        .map_err(|error| lock_fault(&error))?
    }

    /// Run Dylint, printing the built-driver notice when one was built.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError`] when the installation stopped being usable or
    /// when the child cannot be started. A notice that cannot be written is
    /// logged and does not fail the call: the run it describes already
    /// happened.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        match prepared {
            CheckPrepared::Bootstrap { run, route } => run_bootstrapped(
                BootstrappedRun {
                    prepared: &run,
                    route,
                    args,
                },
                stderr,
                run_prepared,
            ),
            CheckPrepared::Direct(library_path) => run_direct(library_path.as_os_str(), args),
        }
    }
}

/// Read which library set this machine's environment selects.
fn library_mode() -> LibraryMode { mode_for(std::env::var_os("DYLINT_LIBRARY_PATH")) }

/// Choose the library mode for the value `DYLINT_LIBRARY_PATH` held.
///
/// An empty value is what a shell leaves behind for `VAR=`, and treating it as
/// authoritative would suppress the recorded installation for no reason.
fn mode_for(explicit: Option<OsString>) -> LibraryMode {
    explicit
        .filter(|path| !path.is_empty())
        .map_or(LibraryMode::Recorded, LibraryMode::UserManaged)
}

/// Ask the installed bootstrap whether it is ready.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// bootstrap.
///
/// # Errors
///
/// Returns [`BootstrapError::NoDataDirectory`] when the platform data
/// directory cannot be determined, and otherwise whatever
/// [`preflight_recorded_with`] reports for this machine.
fn preflight_recorded(
    inherited_path: Option<&OsStr>,
) -> Result<CheckPrepared, CheckPreflightError> {
    let Some(system_dirs) = SystemBaseDirs::new() else {
        return Err(CheckPreflightError::Bootstrap(
            BootstrapError::NoDataDirectory,
        ));
    };
    preflight_recorded_with(&system_dirs, None, inherited_path)
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
/// usable, [`CheckPreflightError::UserPathHasNoSuite`] or
/// [`CheckPreflightError::UserPathAmbiguous`] when the override names no single
/// suite, and [`BootstrapError::LibrariesMissing`] or
/// [`CheckPreflightError::RecordedPathAmbiguous`] when the recorded library
/// directory no longer names exactly one suite.
fn preflight_recorded_with(
    dirs: &dyn BaseDirs,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<CheckPrepared, CheckPreflightError> {
    preflight_recorded_with_cargo(dirs, OsStr::new("cargo"), library_path, inherited_path)
}

/// [`preflight_recorded_with`] with an explicit `cargo` program.
///
/// The readiness probe runs `cargo dylint --version`, so a test that needs a
/// genuine `PreparedRun` from a host whose installed release may differ
/// supplies its own program here rather than replacing the readiness answer.
///
/// `inherited_path` is this process's `PATH` as the composition root read it.
/// It is forwarded so the bootstrap can extend the child's `PATH` on Windows,
/// and is deliberately not read here: a helper that reads the environment
/// cannot be tested with a value the test chose.
///
/// # Errors
///
/// As for [`preflight_recorded_with`].
fn preflight_recorded_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<CheckPrepared, CheckPreflightError> {
    let options = RunOptions {
        library_path,
        inherited_path,
    };
    let prepared =
        preflight_with_cargo(dirs, cargo, options).map_err(CheckPreflightError::Bootstrap)?;
    // The approved path's ownership is decided here, once, and travels with
    // the prepared value: the run re-check has to ask the same question, and
    // re-deriving the answer from the environment could not.
    let route = library_path.map_or(LibraryRoute::Recorded, |_| LibraryRoute::UserManaged);
    suite_rule(route, prepared.library_dir())?;
    Ok(CheckPrepared::Bootstrap {
        run: prepared,
        route,
    })
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
/// finds one, [`BootstrapError::NoDataDirectory`] when the platform data
/// directory cannot be determined, [`CheckPreflightError::UserPathHasNoSuite`]
/// or [`CheckPreflightError::UserPathAmbiguous`] when no installation is
/// recorded and the path names no single suite, and a bootstrap I/O error when
/// a directory on the path cannot be read.
fn preflight_user_managed(
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<CheckPrepared, CheckPreflightError> {
    let system_dirs = SystemBaseDirs::new();
    preflight_user_managed_in(
        system_dirs.as_ref().map(|dirs| dirs as &dyn BaseDirs),
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
/// Returns [`BootstrapError::NoDataDirectory`] when `system_dirs` is absent,
/// the recorded installation's own defect when one is recorded, and
/// [`CheckPreflightError::UserPathHasNoSuite`] or
/// [`CheckPreflightError::UserPathAmbiguous`] when no installation is recorded
/// and `path` names no single suite.
fn preflight_user_managed_in(
    system_dirs: Option<&dyn BaseDirs>,
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<CheckPrepared, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(BootstrapError::NoDataDirectory.into());
    };
    match preflight_recorded_with(dirs, Some(path), inherited_path) {
        Ok(prepared) => Ok(prepared),
        // No record means the legacy route: the user's path is the whole
        // installation. Any other defect is the installation's to report.
        Err(CheckPreflightError::Bootstrap(BootstrapError::NotInstalled { .. })) => {
            // The constructor repeats the inspection the readiness check above
            // performed, so the value it returns is evidence Dylint may run.
            Ok(CheckPrepared::Direct(DirectLibraryPath::new(
                path.to_owned(),
            )?))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
