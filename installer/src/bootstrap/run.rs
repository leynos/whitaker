//! `whitaker-installer run`: validate the bootstrap and run `cargo dylint`.
//!
//! The run is split into a query, [`prepare`], which checks the installation
//! and resolves the Dylint environment without side effects, and a command,
//! [`execute`], which creates the driver slot and holds its lock across the
//! child process so concurrent cold runs build the driver once.

use std::{
    ffi::{OsStr, OsString},
    process::{Command, ExitStatus},
    time::Instant,
};

use camino::Utf8PathBuf;

use super::{
    BootstrapLayout,
    child::check_cargo_dylint,
    driver::DriverSlot,
    error::BootstrapError,
    metadata::BootstrapMetadata,
    probe::is_present_dir,
    telemetry::{DriverPath, child_label, failure_label, log_run, record_run},
};
use crate::{clippy_snapshot::check_installed, dirs::BaseDirs};

/// A validated installation, ready to run.
#[derive(Debug, Clone)]
pub(super) struct PreparedRun {
    pub(super) library_dir: Utf8PathBuf,
    pub(super) snapshot: Utf8PathBuf,
    pub(super) slot: DriverSlot,
}

/// What a finished run reports to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOutcome {
    /// The exit code this process should end with: the child's own status.
    pub code: i32,
    /// Whether this run left a newly built driver in the slot.
    ///
    /// Judged from the driver file's identity before and after the child, so
    /// it holds for a rebuild on a warm run too.
    pub built_driver: bool,
}

/// Run `cargo dylint` with the installed bootstrap and report its outcome.
///
/// # Errors
///
/// Returns a [`BootstrapError`] when the installation is incomplete or
/// invalid, or when `cargo dylint` cannot be started. Lint failures are not
/// errors: they come back as the child's non-zero exit code.
pub fn run(dirs: &dyn BaseDirs, args: &[OsString]) -> Result<RunOutcome, BootstrapError> {
    run_with_cargo(dirs, OsStr::new("cargo"), args)
}

/// [`run`] with an explicit `cargo` program, for tests and diagnostics.
///
/// # Errors
///
/// As for [`run`].
#[tracing::instrument(name = "bootstrap.run", skip_all)]
pub fn run_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    run_resolved(dirs, cargo, args)
}

/// Resolve the installation and run against it.
///
/// The instrumented entry point above stays a single call: the span's expansion
/// and this function's branching together exceed the complexity budget.
fn run_resolved(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    let started = Instant::now();
    match BootstrapLayout::from_dirs(dirs).and_then(|layout| prepare(&layout, cargo)) {
        Ok(prepared) => super::driver_run::execute(&prepared, cargo, args),
        Err(error) => finish(DriverPath::Unresolved, Err(error), started),
    }
}

/// Check the installation and resolve the Dylint environment.
fn prepare(layout: &BootstrapLayout, cargo: &OsStr) -> Result<PreparedRun, BootstrapError> {
    let metadata = BootstrapMetadata::read(&layout.metadata_path())?;
    let library_dir = metadata.library_dir.resolve(layout.data_root());
    if !is_present_dir(&library_dir)? {
        return Err(BootstrapError::LibrariesMissing {
            path: library_dir.to_string(),
        });
    }
    let snapshot = layout.snapshot(&metadata.snapshot_commit);
    check_installed(&snapshot, &metadata.snapshot_commit)?;
    check_cargo_dylint(cargo, &metadata.dylint_version)?;
    let slot = DriverSlot::new(
        &layout.driver_dir(&metadata.dylint_version, &metadata.snapshot_commit),
        &metadata.toolchain,
    );
    Ok(PreparedRun {
        library_dir,
        snapshot,
        slot,
    })
}

/// Record and log a finished run, then hand its result back.
///
/// The outcome is a bounded label; the error itself reaches the user through
/// the returned result rather than the log.
pub(super) fn finish(
    path: DriverPath,
    result: Result<RunOutcome, BootstrapError>,
    started: Instant,
) -> Result<RunOutcome, BootstrapError> {
    let elapsed = started.elapsed();
    let outcome = result
        .as_ref()
        .map_or_else(failure_label, |run| child_label(run.code));
    record_run(path, outcome, elapsed);
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    log_run(path, outcome, result.is_ok(), elapsed_ms);
    result
}
/// Start `cargo dylint` with the bootstrap's variables and wait for it.
///
/// Only the three Dylint variables are overwritten; everything else,
/// including `RUSTC_WRAPPER` and `RUSTFLAGS`, reaches the child unchanged.
#[tracing::instrument(name = "bootstrap.child", skip_all)]
pub(super) fn spawn(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<ExitStatus, BootstrapError> {
    Command::new(cargo)
        .arg("dylint")
        .args(args)
        .env("DYLINT_LIBRARY_PATH", &prepared.library_dir)
        .env("DYLINT_CLIPPY_REPO", &prepared.snapshot)
        .env("DYLINT_DRIVER_PATH", prepared.slot.dir())
        .status()
        .map_err(|error| BootstrapError::Spawn {
            reason: error.to_string(),
        })
}

#[cfg(all(test, unix))]
#[path = "run_tests.rs"]
mod tests;
