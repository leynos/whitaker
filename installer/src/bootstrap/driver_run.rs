//! Driver selection and execution for `whitaker-installer run`.
//!
//! Split from [`super::run`] to keep each file inside the repository's
//! 400-line limit. This module owns the lock protocol: a ready driver is
//! shared, and anything else is built once under the exclusive lock while
//! every other run queues behind it.

use std::{
    ffi::{OsStr, OsString},
    fs::File,
    time::Instant,
};

use super::{
    BootstrapError,
    child::exit_code,
    driver::{DriverIdentity, DriverSlot},
    run::{PreparedRun, RunOutcome, finish, spawn},
    snapshot_check::verify_snapshot,
    telemetry::{
        BuildOutcome,
        DriverPath,
        LockMode,
        failure_label,
        log_driver_changed,
        log_using_ready_driver,
        record_driver_build,
        record_lock_failure,
        record_lock_wait,
    },
};

/// Run the child under the driver lock, recording the run's path and outcome.
pub(super) fn execute(
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
    if slot.is_ready().map_err(unresolved)?
        && let Some(outcome) = warm_or_none(DriverPath::Warm, prepared, cargo, args)?
    {
        return Ok((DriverPath::Warm, outcome));
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

/// Record a rebuilt driver's verification outcome in the slot.
///
/// A verified replacement is marked ready so other runs may share it; a failed
/// verification clears the marker so the next cold run rebuilds from scratch.
fn record_verification(slot: &DriverSlot, verified: &Result<(), BootstrapError>) -> BuildOutcome {
    if verified.is_ok() {
        mark_rebuilt_ready(slot)
    } else {
        clear_rebuilt_ready(slot)
    }
}

/// Mark a verified rebuild ready, reporting but not returning a marker failure.
fn mark_rebuilt_ready(slot: &DriverSlot) -> BuildOutcome {
    if let Err(error) = slot.mark_ready() {
        log_ready_mark_failure(&error);
    }
    BuildOutcome::Built
}

/// Clear the ready marker after a failed verification.
fn clear_rebuilt_ready(slot: &DriverSlot) -> BuildOutcome {
    if let Err(error) = slot.clear_ready() {
        log_ready_clear_failure(&error);
    }
    BuildOutcome::VerifyFailed
}

/// Report a ready marker that could not be written.
fn log_ready_mark_failure(error: &BootstrapError) {
    tracing::warn!(
        failure = failure_label(error),
        "could not record the rebuilt driver as ready"
    );
}

/// Report a ready marker that could not be cleared.
fn log_ready_clear_failure(error: &BootstrapError) {
    tracing::warn!(
        failure = failure_label(error),
        "could not mark the driver cold after a failed verification"
    );
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
    match slot.has_driver() {
        Ok(true) => mark_ready_outcome(slot),
        Ok(false) => report_missing_driver(slot),
        Err(error) => report_unreadable_driver(&error),
    }
}

/// Report a child that exited without leaving a driver behind.
fn report_missing_driver(slot: &DriverSlot) -> BuildOutcome {
    tracing::warn!(
        toolchain = slot.toolchain(),
        "cargo dylint exited without building a driver"
    );
    BuildOutcome::NotBuilt
}

/// Report a driver slot that could not be inspected after the child ran.
fn report_unreadable_driver(error: &BootstrapError) -> BuildOutcome {
    tracing::warn!(
        failure = failure_label(error),
        "could not record the driver as ready"
    );
    BuildOutcome::Unrecorded
}

/// Mark the slot ready after the child left a driver behind.
fn mark_ready_outcome(slot: &DriverSlot) -> BuildOutcome {
    match slot.mark_ready() {
        Ok(()) => BuildOutcome::Built,
        Err(error) => {
            tracing::warn!(
                failure = failure_label(&error),
                "could not record the driver as ready"
            );
            BuildOutcome::Unrecorded
        }
    }
}
