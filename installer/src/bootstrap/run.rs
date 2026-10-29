//! `whitaker-installer run`: validate the bootstrap and run `cargo dylint`.
//!
//! The run is split into a query, [`prepare`], which checks the installation
//! and resolves the Dylint environment without side effects, and a command,
//! [`execute`], which creates the driver slot and holds its lock across the
//! child process so concurrent cold runs build the driver once.

use super::BootstrapLayout;
use super::driver::DriverSlot;
use super::error::BootstrapError;
use super::metadata::BootstrapMetadata;
use super::probe::is_present_dir;
use super::telemetry::{
    BuildOutcome, DriverPath, LockMode, record_driver_build, record_lock_wait, record_run,
};
use crate::clippy_snapshot::{
    ClippySnapshotManifest, GenerationMetadata, check_installed, verify_repository,
};
use crate::dirs::BaseDirs;
use crate::git::CommitSha;
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

/// Run `cargo dylint` with the installed bootstrap and return its exit code.
///
/// # Errors
///
/// Returns a [`BootstrapError`] when the installation is incomplete or
/// invalid, or when `cargo dylint` cannot be started. Lint failures are not
/// errors: they come back as the child's non-zero exit code.
pub fn run(dirs: &dyn BaseDirs, args: &[OsString]) -> Result<i32, BootstrapError> {
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
) -> Result<i32, BootstrapError> {
    let layout = BootstrapLayout::from_dirs(dirs)?;
    let prepared = prepare(&layout, cargo)?;
    execute(&prepared, cargo, args)
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

/// Run the child under the driver lock.
///
/// A ready driver is shared. Otherwise the exclusive lock is taken and the
/// state re-checked, because another process may have finished the build
/// while this one waited. The snapshot gets its full verification only on
/// that cold path, the one place Dylint reads it.
fn execute(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<i32, BootstrapError> {
    let slot = &prepared.slot;
    slot.create()?;
    if slot.is_ready()? {
        let _lock = timed_lock(LockMode::Shared, || slot.lock_shared())?;
        return run_warm(prepared, cargo, args, DriverPath::Warm);
    }
    {
        let _exclusive = timed_lock(LockMode::Exclusive, || slot.lock_exclusive())?;
        if !slot.is_ready()? {
            return run_cold(prepared, cargo, args);
        }
    }
    // Another process built the driver while this one waited. Readiness never
    // reverts, so trade the exclusive lock for a shared one rather than make
    // every run that queued behind the build wait for the others in turn.
    let _lock = timed_lock(LockMode::Shared, || slot.lock_shared())?;
    run_warm(prepared, cargo, args, DriverPath::WarmAfterWait)
}

/// Take a driver lock, recording how long it took to acquire.
fn timed_lock(
    mode: LockMode,
    acquire: impl FnOnce() -> Result<File, BootstrapError>,
) -> Result<File, BootstrapError> {
    let started = Instant::now();
    let lock = acquire()?;
    record_lock_wait(mode, started.elapsed());
    Ok(lock)
}

/// Run the child against a driver that is already ready.
fn run_warm(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
    path: DriverPath,
) -> Result<i32, BootstrapError> {
    tracing::debug!(
        driver = path.label(),
        toolchain = prepared.slot.toolchain(),
        "using the ready driver"
    );
    record_run(path);
    spawn(prepared, cargo, args).map(exit_code)
}

/// Verify the snapshot, run the child so Dylint builds the driver, and record
/// the driver as ready if it now exists. Call only under the exclusive lock.
fn run_cold(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<i32, BootstrapError> {
    tracing::info!(
        toolchain = prepared.slot.toolchain(),
        "building the Dylint driver on first use"
    );
    record_run(DriverPath::Cold);
    let started = Instant::now();
    verify_snapshot(prepared)?;
    let status = spawn(prepared, cargo, args)?;
    let outcome = record_readiness(&prepared.slot);
    record_driver_build(outcome, started.elapsed());
    Ok(exit_code(status))
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
            tracing::warn!(error = %error, "could not record the driver as ready");
            BuildOutcome::Unrecorded
        }
    }
}

/// Fully verify the snapshot against its own installation record.
fn verify_snapshot(prepared: &PreparedRun) -> Result<(), BootstrapError> {
    let generation = GenerationMetadata::read(&prepared.snapshot)?;
    let invalid = |field: &str, error: &dyn std::fmt::Display| {
        crate::clippy_snapshot::ClippySnapshotError::invalid(
            &prepared.snapshot,
            format!("its metadata {field} is not a commit: {error}"),
        )
    };
    let manifest = ClippySnapshotManifest {
        upstream: String::new(),
        commit: CommitSha::try_from(generation.commit.as_str())
            .map_err(|e| invalid("commit", &e))?,
        toolchain: generation.toolchain.clone(),
        selected_revision: CommitSha::try_from(generation.selected_revision.as_str())
            .map_err(|e| invalid("selected_revision", &e))?,
        dylint_version: generation.dylint_version.clone(),
    };
    verify_repository(&prepared.snapshot, &manifest)?;
    Ok(())
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

/// Require the `cargo-dylint` on `PATH` to be the bootstrapped release.
fn check_cargo_dylint(cargo: &OsStr, expected: &str) -> Result<(), BootstrapError> {
    let output = Command::new(cargo)
        .args(["dylint", "--version"])
        .output()
        .map_err(|error| BootstrapError::CargoDylint {
            reason: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(BootstrapError::CargoDylint {
            reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let found = stdout.split_whitespace().last().unwrap_or_default();
    if found == expected {
        Ok(())
    } else {
        Err(BootstrapError::DylintVersion {
            expected: expected.to_owned(),
            found: found.to_owned(),
        })
    }
}

/// Map a child's status to the code this process should exit with.
///
/// A normal exit keeps its code exactly. A Unix process killed by a signal
/// has no code, so it maps to `128 + signal`, the value a shell reports.
#[must_use]
pub fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

#[cfg(all(test, unix))]
#[path = "run_tests.rs"]
mod tests;
