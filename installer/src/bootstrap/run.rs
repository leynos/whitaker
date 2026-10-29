//! `whitaker-installer run`: validate the bootstrap and run `cargo dylint`.
//!
//! The run is split into a query, [`prepare`], which checks the installation
//! and resolves the Dylint environment without side effects, and a command,
//! [`execute`], which creates the driver slot and holds its lock across the
//! child process so concurrent cold runs build the driver once.

use super::BootstrapLayout;
use super::child::{check_cargo_dylint, exit_code};
use super::driver::{DriverIdentity, DriverSlot};
use super::error::BootstrapError;
use super::metadata::BootstrapMetadata;
use super::probe::is_present_dir;
use super::snapshot_check::verify_snapshot;
use super::telemetry::{
    BuildOutcome, DriverPath, LockMode, OUTCOME_OK, failure_label, record_driver_build,
    record_lock_failure, record_lock_wait, record_run,
};
use crate::clippy_snapshot::check_installed;
use crate::dirs::BaseDirs;
use camino::Utf8PathBuf;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::process::{Command, ExitStatus};
use std::time::Instant;

/// A validated installation, ready to run.
#[derive(Debug, Clone)]
struct PreparedRun {
    library_dir: Utf8PathBuf,
    snapshot: Utf8PathBuf,
    slot: DriverSlot,
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
pub fn run_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    let started = Instant::now();
    match BootstrapLayout::from_dirs(dirs).and_then(|layout| prepare(&layout, cargo)) {
        Ok(prepared) => execute(&prepared, cargo, args),
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

/// Run the child under the driver lock, recording the run's path and outcome.
fn execute(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    let started = Instant::now();
    match choose_and_run(prepared, cargo, args) {
        Ok((path, outcome)) => finish(path, Ok(outcome), started),
        Err((path, error)) => finish(path, Err(error), started),
    }
}

/// A result tagged with how far the run got towards the driver.
type Reached<T> = Result<(DriverPath, T), (DriverPath, BootstrapError)>;

/// Tag `result` with the driver path it was produced on.
fn reached<T>(path: DriverPath, result: Result<T, BootstrapError>) -> Reached<T> {
    result
        .map(|value| (path, value))
        .map_err(|error| (path, error))
}

/// Choose the driver path and run the child on it.
///
/// A ready driver is shared. Otherwise the exclusive lock is taken and the
/// state re-checked, because another process may have finished the build
/// while this one waited. The snapshot gets its full verification only on
/// that cold path, the one place Dylint reads it.
fn choose_and_run(prepared: &PreparedRun, cargo: &OsStr, args: &[OsString]) -> Reached<RunOutcome> {
    let slot = &prepared.slot;
    let unresolved = |error| (DriverPath::Unresolved, error);
    slot.create().map_err(unresolved)?;
    if slot.is_ready().map_err(unresolved)? {
        if let Some(outcome) = warm_or_none(DriverPath::Warm, prepared, cargo, args)? {
            return Ok((DriverPath::Warm, outcome));
        }
    }
    let _exclusive =
        timed_lock(LockMode::Exclusive, || slot.lock_exclusive()).map_err(unresolved)?;
    if !slot.is_ready().map_err(unresolved)? {
        return reached(DriverPath::Cold, run_cold(prepared, cargo, args));
    }
    // Another process built and verified the driver while this one waited.
    // Trade the exclusive lock for a shared one rather than make every run
    // that queued behind the build wait for the others in turn.
    drop(_exclusive);
    match warm_or_none(DriverPath::WarmAfterWait, prepared, cargo, args)? {
        Some(outcome) => Ok((DriverPath::WarmAfterWait, outcome)),
        // Replaced again in between: build under the exclusive lock.
        None => {
            let _exclusive = timed_lock(LockMode::Exclusive, || slot.lock_exclusive())
                .map_err(|error| (DriverPath::Cold, error))?;
            reached(DriverPath::Cold, run_cold(prepared, cargo, args))
        }
    }
}

/// [`run_warm`] tagged with `path`, keeping its "not ready after all" answer.
fn warm_or_none(
    path: DriverPath,
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<Option<RunOutcome>, (DriverPath, BootstrapError)> {
    run_warm(prepared, cargo, args).map_err(|error| (path, error))
}

/// Record and log a finished run, then hand its result back.
///
/// The outcome is a bounded label; the error itself reaches the user through
/// the returned result rather than the log.
fn finish(
    path: DriverPath,
    result: Result<RunOutcome, BootstrapError>,
    started: Instant,
) -> Result<RunOutcome, BootstrapError> {
    let elapsed = started.elapsed();
    let outcome = result.as_ref().map_or_else(failure_label, |_| OUTCOME_OK);
    record_run(path, outcome, elapsed);
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    if result.is_ok() {
        tracing::debug!(
            driver = path.label(),
            outcome,
            elapsed_ms,
            "bootstrap run finished"
        );
    } else {
        tracing::warn!(
            driver = path.label(),
            outcome,
            elapsed_ms,
            "bootstrap run failed"
        );
    }
    result
}

/// Take a driver lock, recording how long it took or that it failed.
fn timed_lock(
    mode: LockMode,
    acquire: impl FnOnce() -> Result<File, BootstrapError>,
) -> Result<File, BootstrapError> {
    let started = Instant::now();
    let lock = acquire().inspect_err(|_| record_lock_failure(mode))?;
    record_lock_wait(mode, started.elapsed());
    Ok(lock)
}

/// Run the child under the shared lock against the driver recorded as ready.
///
/// Returns `None`, having run nothing, when the driver present under the lock
/// is not the one the marker records: another run has replaced it and not yet
/// verified the snapshot behind the rebuild, so the caller must take the cold
/// path, whose exclusive lock waits for that run to finish.
fn run_warm(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<Option<RunOutcome>, BootstrapError> {
    let slot = &prepared.slot;
    let _lock = timed_lock(LockMode::Shared, || slot.lock_shared())?;
    // Read the identity first, then compare it with the marker: a replacement
    // after this read changes the identity the child leaves, which the
    // comparison below catches.
    let before = slot.driver_identity()?;
    if before.is_none() || slot.ready_identity()? != before {
        tracing::debug!(
            toolchain = slot.toolchain(),
            "the driver changed since it was verified"
        );
        return Ok(None);
    }
    tracing::debug!(toolchain = slot.toolchain(), "using the ready driver");
    // Dylint re-probes a cached driver and rebuilds it if the probe fails, so
    // a warm run can still build; compare identities rather than assume.
    let started = Instant::now();
    let status = spawn(prepared, cargo, args)?;
    let built_driver = was_replaced(slot, before);
    if built_driver {
        check_warm_rebuild(prepared, started)?;
    }
    Ok(Some(RunOutcome {
        code: exit_code(status),
        built_driver,
    }))
}

/// Verify the snapshot a warm run's rebuild just read, before trusting it.
///
/// Full verification takes seconds on the real snapshot, so warm runs do not
/// pay it up front; only the rare warm run whose child rebuilt the driver
/// does. Until it succeeds, the marker still names the old driver, so other
/// runs treat the replacement as unverified and take the cold path. If the
/// snapshot fails, the run fails with that error and the slot is marked cold,
/// and the next cold run removes the driver before rebuilding, so nothing
/// built from an unverified snapshot is reported or reused.
fn check_warm_rebuild(prepared: &PreparedRun, started: Instant) -> Result<(), BootstrapError> {
    tracing::warn!(
        toolchain = prepared.slot.toolchain(),
        "Dylint rebuilt a ready driver; verifying the snapshot it read"
    );
    let verified = verify_snapshot(&prepared.snapshot);
    let outcome = if verified.is_ok() {
        // Record the verified replacement, so other runs may share it.
        if let Err(error) = prepared.slot.mark_ready() {
            tracing::warn!(
                failure = failure_label(&error),
                "could not record the rebuilt driver as ready"
            );
        }
        BuildOutcome::Built
    } else {
        if let Err(error) = prepared.slot.clear_ready() {
            tracing::warn!(
                failure = failure_label(&error),
                "could not mark the driver cold after a failed verification"
            );
        }
        BuildOutcome::VerifyFailed
    };
    record_driver_build(outcome, started.elapsed());
    verified
}

/// Return whether the child persisted a driver other than `before`.
///
/// A slot that cannot be re-read after the child counts as unchanged: the
/// child's exit status is the result that matters, and the warning says why
/// no build was reported.
fn was_replaced(slot: &DriverSlot, before: Option<DriverIdentity>) -> bool {
    match slot.driver_identity() {
        Ok(after) => after.is_some() && after != before,
        Err(error) => {
            tracing::warn!(
                failure = failure_label(&error),
                "could not re-read the driver after the run"
            );
            false
        }
    }
}

/// Verify the snapshot, run the child so Dylint builds the driver, and record
/// the driver as ready if it now exists. Call only under the exclusive lock.
fn run_cold(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    tracing::info!(
        toolchain = prepared.slot.toolchain(),
        "building the Dylint driver on first use"
    );
    let started = Instant::now();
    // An unmarked driver may already exist, left by an interrupted build or by
    // a warm rebuild whose snapshot then failed verification. Remove it after
    // verifying, so Dylint always builds from the snapshot just checked.
    let before = prepared.slot.driver_identity()?;
    let built = verify_snapshot(&prepared.snapshot)
        .and_then(|()| prepared.slot.remove_driver())
        .and_then(|()| spawn(prepared, cargo, args));
    let outcome = match &built {
        Ok(_) => record_readiness(&prepared.slot),
        Err(BootstrapError::Snapshot(_)) => BuildOutcome::VerifyFailed,
        Err(BootstrapError::Spawn { .. }) => BuildOutcome::SpawnFailed,
        Err(_) => BuildOutcome::Unrecorded,
    };
    record_driver_build(outcome, started.elapsed());
    built.map(|status| RunOutcome {
        code: exit_code(status),
        built_driver: was_replaced(&prepared.slot, before),
    })
}

/// Mark the slot ready when the child left a driver behind.
///
/// Failures here are reported but not returned: the child has already run,
/// and its exit status matters more than the marker. An unmarked slot is
/// simply cold again next time.
fn record_readiness(slot: &DriverSlot) -> BuildOutcome {
    match slot.has_driver().and_then(|has_driver| {
        has_driver
            .then(|| slot.mark_ready())
            .transpose()
            .map(|marked| marked.is_some())
    }) {
        Ok(true) => BuildOutcome::Built,
        Ok(false) => {
            tracing::warn!(
                toolchain = slot.toolchain(),
                "cargo dylint exited without building a driver"
            );
            BuildOutcome::NotBuilt
        }
        Err(error) => {
            tracing::warn!(
                failure = failure_label(&error),
                "could not record the driver as ready"
            );
            BuildOutcome::Unrecorded
        }
    }
}

/// Start `cargo dylint` with the bootstrap's variables and wait for it.
///
/// Only the three Dylint variables are overwritten; everything else,
/// including `RUSTC_WRAPPER` and `RUSTFLAGS`, reaches the child unchanged.
fn spawn(
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
