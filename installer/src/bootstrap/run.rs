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
    child_path::child_path,
    driver::DriverSlot,
    error::BootstrapError,
    metadata::BootstrapMetadata,
    probe::is_present_dir,
    telemetry::{DriverPath, child_label, failure_label, log_run, record_run},
};
use crate::{clippy_snapshot::check_installed, dirs::BaseDirs};

/// A validated installation, ready to run.
///
/// This is the value a successful [`preflight`] returns and the only thing
/// [`run_prepared`] will run. Its fields stay private so a caller can obtain
/// one only by asking the installation whether it is ready: the type is the
/// evidence that the checks happened.
#[derive(Debug, Clone)]
pub struct PreparedRun {
    /// The value the child receives as `DYLINT_LIBRARY_PATH`.
    ///
    /// An `OsString` rather than a path so a caller-supplied override, which
    /// may hold several directories, passes through untouched.
    pub(super) library_dir: OsString,
    pub(super) snapshot: Utf8PathBuf,
    pub(super) slot: DriverSlot,
    /// The `PATH` to give the child in place of the inherited one, if any.
    pub(super) child_path: Option<OsString>,
}

impl PreparedRun {
    /// Report the `DYLINT_LIBRARY_PATH` value the child will receive.
    ///
    /// Exposed so a caller that needs more than "the installation is ready" —
    /// `whitaker check` requires the suite library itself, which a plain
    /// directory check does not establish — can inspect what was approved
    /// without re-deriving it.
    #[must_use]
    pub fn library_dir(&self) -> &OsStr { &self.library_dir }
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
    /// The caller's own `PATH`, which the child inherits.
    ///
    /// Passing it lets the bootstrap append the pinned toolchain's `bin` on
    /// Windows, where Dylint's driver probe needs it (see `child_path`). The
    /// caller reads the environment, once, at the production edge; this module
    /// never does. `None` leaves the child's `PATH` exactly as inherited.
    pub inherited_path: Option<&'a OsStr>,
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

/// Resolve the installation and run against it.
///
/// The instrumented entry points above stay single calls: the span's expansion
/// and this function's branching together exceed the complexity budget.
fn run_inner(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    options: RunOptions<'_>,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    let started = Instant::now();
    match preflight_with_cargo(dirs, cargo, options) {
        Ok(prepared) => run_prepared_with_cargo(&prepared, cargo, args),
        Err(error) => finish(DriverPath::Unresolved, Err(error), started),
    }
}

/// Check the installation and resolve the Dylint environment without starting
/// a lint run.
///
/// This is the readiness query: it validates the recorded metadata, the lint
/// library directory, the Clippy snapshot and the installed `cargo-dylint`
/// release, and returns a [`PreparedRun`] only when all four hold. It is not
/// free of subprocesses: once the filesystem checks pass, the version probe
/// starts `cargo dylint --version`. A caller that wants to know whether this
/// machine can lint asks here rather than starting Dylint and interpreting its
/// exit status, which is ambiguous.
///
/// # Errors
///
/// Returns a [`BootstrapError`] naming the first defect found. Every variant
/// is actionable: a caller can tell the repairable ones (the installation is
/// incomplete or stale) from the host faults (the data directory is unknown,
/// or the filesystem failed).
pub fn preflight(
    dirs: &dyn BaseDirs,
    options: RunOptions<'_>,
) -> Result<PreparedRun, BootstrapError> {
    preflight_with_cargo(dirs, OsStr::new("cargo"), options)
}

/// [`preflight`] with an explicit `cargo` program, for tests and diagnostics.
///
/// # Errors
///
/// As for [`preflight`].
pub fn preflight_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    options: RunOptions<'_>,
) -> Result<PreparedRun, BootstrapError> {
    BootstrapLayout::from_dirs(dirs).and_then(|layout| prepare(&layout, cargo, options))
}

/// Run `cargo dylint` against an installation [`preflight`] has approved.
///
/// `args` are the arguments to `cargo dylint` itself: the options Whitaker
/// owns, then `--`, then the arguments the user forwarded to `cargo check`.
///
/// # Errors
///
/// Returns a [`BootstrapError`] when the driver slot cannot be prepared or
/// `cargo dylint` cannot be started. A lint failure is not an error: it comes
/// back as the child's non-zero exit code.
pub fn run_prepared(
    prepared: &PreparedRun,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    run_prepared_with_cargo(prepared, OsStr::new("cargo"), args)
}

/// [`run_prepared`] with an explicit `cargo` program, for tests and
/// diagnostics.
///
/// # Errors
///
/// As for [`run_prepared`].
pub fn run_prepared_with_cargo(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<RunOutcome, BootstrapError> {
    super::driver_run::execute(prepared, cargo, args)
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
    let child_path = options
        .inherited_path
        .and_then(|inherited| child_path(&metadata.toolchain, inherited));
    Ok(PreparedRun {
        library_dir,
        snapshot,
        slot,
        child_path,
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
/// including `RUSTC_WRAPPER` and `RUSTFLAGS`, reaches the child unchanged. The
/// one other change is on Windows, where the pinned toolchain's `bin` is
/// appended to `PATH` (see `child_path`).
#[tracing::instrument(name = "bootstrap.child", skip_all)]
pub(super) fn spawn(
    prepared: &PreparedRun,
    cargo: &OsStr,
    args: &[OsString],
) -> Result<ExitStatus, BootstrapError> {
    child_command(prepared, cargo, args)
        .status()
        .map_err(|error| BootstrapError::Spawn {
            reason: error.to_string(),
        })
}

/// Build the `cargo dylint` command with the bootstrap's environment.
fn child_command(prepared: &PreparedRun, cargo: &OsStr, args: &[OsString]) -> Command {
    let mut command = Command::new(cargo);
    command
        .arg("dylint")
        .args(args)
        .env("DYLINT_LIBRARY_PATH", &prepared.library_dir)
        .env("DYLINT_CLIPPY_REPO", &prepared.snapshot)
        .env("DYLINT_DRIVER_PATH", prepared.slot.dir());
    if let Some(path) = &prepared.child_path {
        command.env("PATH", path);
    }
    command
}

#[cfg(all(test, unix))]
#[path = "run_fixture.rs"]
mod fixture;

#[cfg(all(test, unix))]
#[path = "run_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "run_api_tests.rs"]
mod api_tests;
