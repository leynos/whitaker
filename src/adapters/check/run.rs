//! Running an approved `check`: the two routes, and the status they report.
//!
//! Readiness approves a library path, and this module starts Dylint against
//! it. Both routes re-check the path immediately before the child, because the
//! filesystem can change in between; both report the child's status unchanged.
//! The built-driver notice is written after the run, so a stream that refuses
//! it never replaces the status the run produced.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    process::Command,
};

use whitaker_installer::bootstrap::{BUILT_DRIVER_NOTICE, BootstrapError, PreparedRun, RunOutcome};

use super::suite_rule;
use crate::{
    adapters::child_exit_code,
    ports::{CheckRunError, LibraryRoute},
};

/// One bootstrapped run's inputs.
///
/// Grouped because they travel together: the prepared run is the value a
/// successful preflight returned, the route names which record approved its
/// library path, and the arguments are the caller's own. The run function
/// judges all three before it starts anything.
///
/// Every field is a reference, so copying the value costs nothing and the
/// by-value parameter does not move anything the caller still needs.
#[derive(Clone, Copy)]
pub(super) struct BootstrappedRun<'a> {
    /// The approved run whose library path must still name one suite.
    pub(super) prepared: &'a PreparedRun,
    /// Which record approved that library path.
    pub(super) route: LibraryRoute,
    /// The caller's arguments, forwarded to Dylint unchanged.
    pub(super) args: &'a [OsString],
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
/// The path is judged by [`suite_rule`], using the route that approved it, so
/// a user-managed path that lost its suite keeps the user-path reason rather
/// than being described as a defect of the installed libraries.
///
/// Starting the run is a parameter so a test can present a completed outcome
/// without starting Dylint: the notice precedence is a promise about this
/// function's answer, and the real run would need a stub `cargo` on the test
/// process's `PATH`, which this repository does not mutate. Production passes
/// [`run_prepared`](whitaker_installer::bootstrap::run_prepared).
///
/// # Errors
///
/// Returns [`CheckRunError::Preflight`] when the approved library path no
/// longer names exactly one Whitaker suite, and whatever `start` reports when
/// the driver cannot be prepared or started.
pub(super) fn run_bootstrapped(
    request: BootstrappedRun<'_>,
    stderr: &mut dyn Write,
    start: impl FnOnce(&PreparedRun, &[OsString]) -> Result<RunOutcome, BootstrapError>,
) -> Result<i32, CheckRunError> {
    suite_rule(request.route, request.prepared.library_dir()).map_err(CheckRunError::Preflight)?;
    let outcome = start(request.prepared, request.args)?;
    Ok(completed_status(outcome, stderr))
}

/// Report a completed run's status, announcing a driver this run built.
///
/// The notice is written after the status is known, and a stream that refuses
/// it does not change that status: the lint run already happened, so its status
/// is what the caller asked for. ADR 0017 records that precedence, and keeping
/// it in one function gives the failing-writer case something to test without
/// starting Dylint.
pub(super) fn completed_status(outcome: RunOutcome, stderr: &mut dyn Write) -> i32 {
    if outcome.built_driver {
        report_built_driver(stderr);
    }
    outcome.code
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
/// The path is re-checked here for the same reason the bootstrapped route
/// re-checks its own: readiness approved it moments earlier, and a suite
/// removed or duplicated in between would make Dylint exit `1`, which is
/// indistinguishable from a lint denial. Refusing under the user-path reason
/// keeps the run from blaming the user's code for a directory that changed
/// under it, and no repair is spent on a path the user chose.
///
/// # Errors
///
/// Returns [`CheckRunError::Preflight`] when the approved path no longer names
/// exactly one Whitaker suite, and [`CheckRunError::Spawn`] when `cargo dylint`
/// cannot be started. A child that starts and exits non-zero is not an error:
/// its status is the lint result and is returned unchanged.
pub(super) fn run_direct(library_path: &OsStr, args: &[OsString]) -> Result<i32, CheckRunError> {
    run_direct_with_cargo(library_path, args, OsStr::new("cargo"))
}

/// [`run_direct`] with an explicit `cargo` program.
///
/// The production path supplies the `cargo` on `PATH`. A test supplies a
/// recording stub so it can prove the refusal happens before the child is
/// started: an assertion on the returned error alone would not show whether a
/// regressed check spawned Dylint first and read its status afterwards. Naming
/// the program here keeps that seam explicit rather than reaching into the
/// process environment.
///
/// # Errors
///
/// As [`run_direct`]; the program named here is what [`CheckRunError::Spawn`]
/// reports when it cannot be started.
pub(super) fn run_direct_with_cargo(
    library_path: &OsStr,
    args: &[OsString],
    cargo: &OsStr,
) -> Result<i32, CheckRunError> {
    suite_rule(LibraryRoute::UserManaged, library_path).map_err(CheckRunError::Preflight)?;
    let status = Command::new(cargo)
        .arg("dylint")
        .args(args)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .status()
        .map_err(|source| CheckRunError::Spawn { source })?;
    Ok(child_exit_code(status))
}
