//! `whitaker-installer run`: validate the bootstrap and run `cargo dylint`.
//!
//! The run is split into a query, [`prepare`], which checks the installation
//! and resolves the Dylint environment without side effects, and a command,
//! [`execute`], which creates the driver slot and holds its lock across the
//! child process so concurrent cold runs build the driver once.

use std::{
    ffi::{OsStr, OsString},
    fs::File,
    process::{Command, ExitStatus},
    time::Instant,
};

use camino::Utf8PathBuf;

use super::{
    BootstrapLayout,
    child::{check_cargo_dylint, exit_code},
    driver::{DriverIdentity, DriverSlot},
    error::BootstrapError,
    metadata::BootstrapMetadata,
    probe::is_present_dir,
    run_readiness::{record_readiness, record_verification},
    snapshot_check::verify_snapshot,
    telemetry::{
        BuildOutcome,
        DriverPath,
        LockMode,
        child_label,
        failure_label,
        log_driver_changed,
        log_run,
        log_using_ready_driver,
        record_driver_build,
        record_lock_failure,
        record_lock_wait,
        record_run,
    },
};
use crate::{clippy_snapshot::check_installed, dirs::BaseDirs};

/// A validated installation, ready to run.
#[derive(Debug, Clone)]
struct PreparedRun {
    library_dir: OsString,
    snapshot: Utf8PathBuf,
    slot: DriverSlot,
}

/// Adjustments a caller makes to a run of the installed bootstrap.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions<'a> {
    /// Exports this library path in place of the one the installation records.
    ///
    /// The root CLI passes the per-toolchain paths it recorded when
    /// installing, so Dylint can still select the toolchain a project
    /// declares. The value is a complete `DYLINT_LIBRARY_PATH`; the caller
    /// checks that its directories exist. `None` keeps the recorded directory.
    pub library_path: Option<&'a OsStr>,
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
    run_with_options(dirs, RunOptions::default(), args)
}

/// [`run`] with caller-supplied adjustments.
///
/// # Errors
///
/// As for [`run`].
#[tracing::instrument(name = "bootstrap.run", skip_all)]
pub fn run_with_options(
    dirs: &dyn BaseDirs,
    options: RunOptions<'_>,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    run_inner(dirs, OsStr::new("cargo"), options, args)
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
    run_inner(dirs, cargo, RunOptions::default(), args)
}

/// Run the bootstrap with every option exposed to the callers above.
///
/// The public entry points differ only in the defaults they supply, so they
/// share this implementation. The span lives on the entry points rather than
/// here because its expansion, added to this function's branching, would
/// exceed the cognitive-complexity budget.
fn run_inner(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    options: RunOptions<'_>,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    let started = Instant::now();
    match BootstrapLayout::from_dirs(dirs).and_then(|layout| prepare(&layout, cargo, options)) {
        Ok(prepared) => execute(&prepared, cargo, args),
        Err(error) => finish(DriverPath::Unresolved, Err(error), started),
    }
}

/// Check the installation and resolve the Dylint environment.
fn prepare(
    layout: &BootstrapLayout,
    cargo: &OsStr,
    options: RunOptions<'_>,
) -> Result<PreparedRun, BootstrapError> {
    let metadata = BootstrapMetadata::read(&layout.metadata_path())?;
    let library_dir = resolve_library_dir(layout, &metadata, options)?;
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

/// Resolve the library directory the child should export.
///
/// A caller-supplied path wins outright: the root CLI forwards the per-toolchain
/// directories it recorded during its own install, and checking the install's
/// single recorded directory instead would reject a perfectly good override.
/// Without an override the recorded directory is used, and must still exist.
///
/// # Errors
///
/// Returns [`BootstrapError::LibrariesMissing`] when the recorded directory is
/// absent, or the inspection error when it exists but cannot be read.
fn resolve_library_dir(
    layout: &BootstrapLayout,
    metadata: &BootstrapMetadata,
    options: RunOptions<'_>,
) -> Result<OsString, BootstrapError> {
    if let Some(path) = options.library_path {
        return Ok(path.to_owned());
    }
    let recorded = metadata.library_dir.resolve(layout.data_root());
    if !is_present_dir(&recorded)? {
        return Err(BootstrapError::LibrariesMissing {
            path: recorded.to_string(),
        });
    }
    Ok(recorded.into_os_string())
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
    let exclusive =
        timed_lock(LockMode::Exclusive, || slot.lock_exclusive()).map_err(unresolved)?;
    if !slot.is_ready().map_err(unresolved)? {
        return reached(DriverPath::Cold, run_cold(prepared, cargo, args));
    }
    // Another process built and verified the driver while this one waited.
    // Trade the exclusive lock for a shared one rather than make every run
    // that queued behind the build wait for the others in turn.
    drop(exclusive);
    if let Some(outcome) = warm_or_none(DriverPath::WarmAfterWait, prepared, cargo, args)? {
        return Ok((DriverPath::WarmAfterWait, outcome));
    }
    // Replaced again in between: build under the exclusive lock.
    let _exclusive = timed_lock(LockMode::Exclusive, || slot.lock_exclusive())
        .map_err(|error| (DriverPath::Cold, error))?;
    reached(DriverPath::Cold, run_cold(prepared, cargo, args))
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
    let outcome = result
        .as_ref()
        .map_or_else(failure_label, |run| child_label(run.code));
    record_run(path, outcome, elapsed);
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    log_run(path, outcome, result.is_ok(), elapsed_ms);
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
    if !is_marked_ready(slot, before)? {
        log_driver_changed(slot.toolchain());
        return Ok(None);
    }
    log_using_ready_driver(slot.toolchain());
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

/// Return whether `before` is still the driver the ready marker records.
fn is_marked_ready(
    slot: &DriverSlot,
    before: Option<DriverIdentity>,
) -> Result<bool, BootstrapError> {
    Ok(before.is_some() && slot.ready_identity()? == before)
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
    let outcome = record_verification(&prepared.slot, &verified);
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

/// Start `cargo dylint` with the bootstrap's variables and wait for it.
///
/// Only the three Dylint variables are overwritten; everything else,
/// including `RUSTC_WRAPPER` and `RUSTFLAGS`, reaches the child unchanged.
#[tracing::instrument(name = "bootstrap.child", skip_all)]
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
