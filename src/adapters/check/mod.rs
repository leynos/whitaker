//! The `check` adapter: readiness inspection, and the Dylint run behind it.
//!
//! The adapter has two routes, and which one applies is decided by the
//! environment rather than by the caller. A machine that manages its own
//! `DYLINT_LIBRARY_PATH` has already said which libraries to load, so Whitaker
//! inspects that path and runs `cargo dylint` directly; on any other machine
//! the installed bootstrap supplies the libraries, the Clippy snapshot and the
//! managed driver, and is asked whether it is ready.
//!
//! Only this module reads that environment variable. The command layer is
//! handed a value, never the process environment.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    process::Command,
};

use repair::{RepairStreams, lock_fault, repair_request, repair_under_lock};
use suite::{recorded_suite_present, suite_present};
use whitaker_installer::{
    bootstrap::{
        BUILT_DRIVER_NOTICE,
        BootstrapError,
        PreparedRun,
        RunOptions,
        preflight,
        run_prepared,
    },
    dirs::{BaseDirs, SystemBaseDirs},
    orchestration::with_install_lock,
};

use super::child_exit_code;
use crate::{
    diagnostics::DiagnosticSettings,
    ports::{
        CheckPreflightError,
        CheckPrepared,
        CheckRepairError,
        CheckRunError,
        CheckServices,
        DirectLibraryPath,
    },
};

mod repair;
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
        match library_mode() {
            LibraryMode::UserManaged(path) => preflight_user_managed(&path),
            LibraryMode::Recorded => preflight_recorded(),
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
        let request = repair_request(settings, std::env::var_os("CI").as_deref());
        with_install_lock(&dirs, || {
            // The installer reserves standard output for machine-readable
            // markers. `check` owns only diagnostics, so markers reach the
            // same stream rather than whatever the user pipes from `check`.
            let mut installer_out = std::io::stderr();
            repair_under_lock(
                &dirs,
                request,
                RepairStreams {
                    installer: &mut installer_out,
                    diagnostics: stderr,
                },
            )
        })
        .map_err(|error| lock_fault(&error))?
    }

    /// Run Dylint, printing the built-driver notice when one was built.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRunError`] when the installation stopped being usable,
    /// when the child cannot be started, or when the notice cannot be written.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        match prepared {
            CheckPrepared::Bootstrap(run) => run_bootstrapped(&run, args, stderr),
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
fn preflight_recorded() -> Result<CheckPrepared, CheckPreflightError> {
    let Some(system_dirs) = SystemBaseDirs::new() else {
        return Err(CheckPreflightError::Bootstrap(
            BootstrapError::NoDataDirectory,
        ));
    };
    preflight_recorded_with(&system_dirs, None)
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
) -> Result<CheckPrepared, CheckPreflightError> {
    // The composition root reads `PATH` once and hands it down as a value; the
    // bootstrap only needs it to extend the child's `PATH` on Windows.
    let inherited_path = std::env::var_os("PATH");
    let options = RunOptions {
        library_path,
        inherited_path: inherited_path.as_deref(),
    };
    let prepared = preflight(dirs, options).map_err(CheckPreflightError::Bootstrap)?;
    match library_path {
        Some(path) => suite_present(path)?,
        None => recorded_suite_present(prepared.library_dir())?,
    }
    Ok(CheckPrepared::Bootstrap(prepared))
}

/// Decide what a user-managed library path needs before Dylint can run.
///
/// A recorded installation still provides the snapshot and driver, so the
/// readiness check runs with the user's path as the override. Only when no
/// installation has been recorded does the user's path have to supply
/// everything, and then it is inspected for a Whitaker suite: without one,
/// Dylint would fail with a status that looks like a lint result.
///
/// # Errors
///
/// Returns the recorded installation's own defect when its readiness check
/// finds one, [`BootstrapError::NoDataDirectory`] when the platform data
/// directory cannot be determined, [`CheckPreflightError::UserPathHasNoSuite`]
/// or [`CheckPreflightError::UserPathAmbiguous`] when no installation is
/// recorded and the path names no single suite, and a bootstrap I/O error when
/// a directory on the path cannot be read.
fn preflight_user_managed(path: &OsStr) -> Result<CheckPrepared, CheckPreflightError> {
    let system_dirs = SystemBaseDirs::new();
    preflight_user_managed_in(system_dirs.as_ref().map(|dirs| dirs as &dyn BaseDirs), path)
}

/// Decide the user-managed route with the platform directories as a value.
///
/// The directories are a parameter rather than a lookup so the undeterminable
/// case is a decision this function makes, and a test can drive it. Without a
/// data directory there is no way to tell whether an installation is recorded,
/// so the route reports the host fault both routes share instead of silently
/// running a path the record might contradict.
fn preflight_user_managed_in(
    system_dirs: Option<&dyn BaseDirs>,
    path: &OsStr,
) -> Result<CheckPrepared, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(BootstrapError::NoDataDirectory.into());
    };
    match preflight_recorded_with(dirs, Some(path)) {
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

/// Run the validated bootstrap and report the child's status.
///
/// The library path is re-checked here because readiness was established
/// before the child could be started: a suite removed in between would make
/// Dylint exit `1`, which is indistinguishable from a lint denial, and a
/// second suite added in between would make it exit `1` for the same reason.
/// Reporting either as a typed provisioning defect lets `check` spend its one
/// repair instead of blaming the user's code.
///
/// # Errors
///
/// Returns [`CheckRunError::Preflight`] when the approved library path no
/// longer names exactly one Whitaker suite, and whatever [`run_prepared`]
/// reports when the driver cannot be prepared or started.
fn run_bootstrapped(
    prepared: &PreparedRun,
    args: &[OsString],
    stderr: &mut dyn Write,
) -> Result<i32, CheckRunError> {
    recorded_suite_present(prepared.library_dir()).map_err(CheckRunError::Preflight)?;
    let outcome = run_prepared(prepared, args)?;
    if outcome.built_driver {
        report_built_driver(stderr);
    }
    Ok(outcome.code)
}

/// Report a driver this run built, without letting the notice change the
/// status.
///
/// The lint run has already happened, so its status is the answer the caller
/// asked for; a notice that never reached the stream is worth a log line, not
/// a replacement status.
fn report_built_driver(stderr: &mut dyn Write) {
    if let Err(error) = writeln!(stderr, "{BUILT_DRIVER_NOTICE}") {
        tracing::warn!(%error, "could not report the built Dylint driver");
    }
}

/// Run `cargo dylint` against a library path the user manages.
///
/// # Errors
///
/// Returns [`CheckRunError::Spawn`] when `cargo dylint` cannot be started. A
/// child that starts and exits non-zero is not an error: its status is the lint
/// result and is returned unchanged.
fn run_direct(library_path: &OsStr, args: &[OsString]) -> Result<i32, CheckRunError> {
    let status = Command::new("cargo")
        .arg("dylint")
        .args(args)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .status()
        .map_err(|source| CheckRunError::Spawn { source })?;
    Ok(child_exit_code(status))
}

#[cfg(test)]
mod tests;
