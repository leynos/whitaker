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

use repair::{lock_fault, repair_under_lock};
use suite::suite_present;
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
    ports::{CheckPreflightError, CheckPrepared, CheckRepairError, CheckRunError, CheckServices},
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
    /// the installation is not ready, [`CheckPreflightError::UserPathHasNoSuite`]
    /// when the user's own path holds no Whitaker suite, and a bootstrap I/O
    /// error when a directory on that path cannot be read.
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
        with_install_lock(&dirs, || repair_under_lock(&dirs, settings, stderr))
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
            CheckPrepared::Direct { library_path } => run_direct(&library_path, args),
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

/// Require the installation's recorded library directory to hold the suite.
///
/// A recorded directory that exists but no longer contains `whitaker_suite` is
/// incomplete, so it is reported as the repairable `LibrariesMissing` defect:
/// reinstalling stages the library, and Dylint would otherwise reject
/// `--lib=whitaker_suite` with a status indistinguishable from a lint denial.
fn recorded_suite_present(library_dir: &OsStr) -> Result<(), CheckPreflightError> {
    if suite::suite_count(library_dir)? > 0 {
        return Ok(());
    }
    Err(BootstrapError::LibrariesMissing {
        path: library_dir.to_string_lossy().into_owned(),
    }
    .into())
}

/// Decide what a user-managed library path needs before Dylint can run.
///
/// A recorded installation still provides the snapshot and driver, so the
/// readiness check runs with the user's path as the override. Only when no
/// installation has been recorded does the user's path have to supply
/// everything, and then it is inspected for a Whitaker suite: without one,
/// Dylint would fail with a status that looks like a lint result.
fn preflight_user_managed(path: &OsStr) -> Result<CheckPrepared, CheckPreflightError> {
    if let Some(dirs) = SystemBaseDirs::new() {
        match preflight_recorded_with(&dirs, Some(path)) {
            Ok(prepared) => return Ok(prepared),
            // No record means the legacy route: the user's path is the whole
            // installation. Any other defect is the installation's to report.
            Err(CheckPreflightError::Bootstrap(BootstrapError::NotInstalled { .. })) => {}
            Err(error) => return Err(error),
        }
    }
    suite_present(path)?;
    Ok(CheckPrepared::Direct {
        library_path: path.to_owned(),
    })
}

/// Run the validated bootstrap and report the child's status.
///
/// The library path is re-checked here because readiness was established
/// before the child could be started: a suite removed in between would make
/// Dylint exit `1`, which is indistinguishable from a lint denial. Reporting
/// the loss as a typed provisioning defect lets `check` spend its one repair
/// instead of blaming the user's code.
///
/// # Errors
///
/// Returns [`CheckRunError::Bootstrap`] when the approved library path no
/// longer holds exactly one Whitaker suite, and whatever
/// [`run_prepared`] reports when the driver cannot be prepared or started.
fn run_bootstrapped(
    prepared: &PreparedRun,
    args: &[OsString],
    stderr: &mut dyn Write,
) -> Result<i32, CheckRunError> {
    prepared_suite_present(prepared.library_dir()).map_err(CheckRunError::Bootstrap)?;
    let outcome = run_prepared(prepared, args)?;
    if outcome.built_driver {
        // The lint run has already happened, so its status is the answer the
        // caller asked for; a notice that never reached the stream is worth a
        // log line, not a replacement status.
        if let Err(error) = writeln!(stderr, "{BUILT_DRIVER_NOTICE}") {
            tracing::warn!(%error, "could not report the built Dylint driver");
        }
    }
    Ok(outcome.code)
}

/// Re-check an approved library path as a bootstrap defect.
///
/// The readiness variants that name the user's path cannot arise here: the
/// path was approved moments earlier, so a loss is an incomplete installation.
fn prepared_suite_present(library_dir: &OsStr) -> Result<(), BootstrapError> {
    match suite_present(library_dir) {
        Ok(()) => Ok(()),
        Err(CheckPreflightError::Bootstrap(error)) => Err(error),
        Err(_) => Err(BootstrapError::LibrariesMissing {
            path: library_dir.to_string_lossy().into_owned(),
        }),
    }
}

/// Run `cargo dylint` against a library path the user manages.
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
