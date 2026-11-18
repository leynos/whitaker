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
    path::Path,
    process::Command,
};

use whitaker_installer::{
    bootstrap::{
        BUILT_DRIVER_NOTICE,
        BootstrapError,
        PreparedRun,
        RunOptions,
        preflight,
        run_prepared,
    },
    dirs::SystemBaseDirs,
    resolution::SUITE_CRATE,
    scanner::parse_library_filename,
};

use super::child_exit_code;
use crate::ports::{CheckPreflightError, CheckPrepared, CheckRunError, CheckServices};

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
            LibraryMode::UserManaged(path) => {
                suite_present(&path)?;
                Ok(CheckPrepared::Direct { library_path: path })
            }
            LibraryMode::Recorded => preflight_recorded(),
        }
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
    // The composition root reads `PATH` once and hands it down as a value; the
    // bootstrap only needs it to extend the child's `PATH` on Windows.
    let inherited_path = std::env::var_os("PATH");
    let options = RunOptions {
        library_path: None,
        inherited_path: inherited_path.as_deref(),
    };
    preflight(&system_dirs, options)
        .map(CheckPrepared::Bootstrap)
        .map_err(CheckPreflightError::Bootstrap)
}

/// Run the validated bootstrap and report the child's status.
fn run_bootstrapped(
    prepared: &PreparedRun,
    args: &[OsString],
    stderr: &mut dyn Write,
) -> Result<i32, CheckRunError> {
    let outcome = run_prepared(prepared, args)?;
    if outcome.built_driver {
        writeln!(stderr, "{BUILT_DRIVER_NOTICE}")
            .map_err(|source| CheckRunError::Output { source })?;
    }
    Ok(outcome.code)
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

/// Confirm that some directory on `path` holds a Whitaker suite library.
///
/// # Errors
///
/// Returns a bootstrap I/O error when a directory cannot be read. That is a
/// host fault rather than a missing suite: reporting it as "the user's path
/// holds nothing" would send them looking for the wrong problem.
fn suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    for directory in std::env::split_paths(path) {
        if holds_suite(&directory)? {
            return Ok(());
        }
    }
    Err(CheckPreflightError::UserPathHasNoSuite)
}

/// Report whether one directory holds a Whitaker suite library.
///
/// A missing directory is not an error: it is one entry of a path list that an
/// earlier install may have removed.
fn holds_suite(directory: &Path) -> Result<bool, CheckPreflightError> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_fault(directory, &error)),
    };
    for found in entries {
        let directory_entry = found.map_err(|error| io_fault(directory, &error))?;
        if is_suite_library(&directory_entry.file_name()) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Report whether a file name is a staged Whitaker suite library.
fn is_suite_library(name: &OsStr) -> bool {
    name.to_str()
        .and_then(parse_library_filename)
        .is_some_and(|(krate, _)| krate.as_str() == SUITE_CRATE)
}

/// Turn a directory read failure into the bootstrap's I/O report.
fn io_fault(directory: &Path, error: &std::io::Error) -> CheckPreflightError {
    BootstrapError::Io {
        path: directory.display().to_string(),
        reason: error.to_string(),
    }
    .into()
}

#[cfg(test)]
mod tests;
