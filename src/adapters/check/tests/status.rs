//! Which status a completed run reports when its notice cannot be written.
//!
//! The built-driver notice is advisory: it says a driver was built on first
//! use and later runs reuse it. It is written after the lint run has finished,
//! so a stream that refuses it must not replace the status the run produced —
//! a lint denial would otherwise be reported as an invocation that merely
//! failed to talk. ADR 0017 records that precedence, and these cases pin both
//! halves of it without starting Dylint.

use whitaker_installer::bootstrap::{BUILT_DRIVER_NOTICE, RunOutcome};

use super::{super::run::completed_status, fixtures::ClosedStream};
#[cfg(unix)]
use super::{
    super::{
        approval::Approval,
        preflight_recorded_with_cargo,
        run::{BootstrappedRun, run_bootstrapped},
    },
    fixtures::{ReadyInstallation, suite_filename},
};

/// Keep the lint status when the built-driver notice cannot be written.
#[test]
fn a_refused_notice_does_not_replace_the_lint_status() {
    let mut closed = ClosedStream;

    let status = completed_status(
        RunOutcome {
            code: 1,
            built_driver: true,
        },
        &mut closed,
    );

    assert_eq!(
        status, 1,
        "a lint denial must survive a notice the stream refuses"
    );
}

/// Give the caller the lint status when the notice is refused.
///
/// The promise belongs to the run boundary, not to [`completed_status`]: the
/// caller must receive the child's `1` from `run_bootstrapped` even when the
/// stream refuses the notice. The run is injected because the real one starts
/// Dylint, and supplying a stub `cargo` would mean mutating this process's
/// `PATH`; the route and the recorded suite are real, so the suite rule runs.
#[cfg(unix)]
#[test]
fn a_caller_keeps_the_completed_status_when_the_notice_is_refused() {
    let installation = ReadyInstallation::new().expect("the fixture installation is ready");
    let recorded = installation
        .dirs
        .layout()
        .data_root()
        .join("whitaker/lints");
    std::fs::write(recorded.join(suite_filename()), b"").expect("the recorded suite can be staged");
    let approval = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("a recorded directory holding the suite must be approved");
    let Approval::Bootstrap { run, route } = approval else {
        panic!("a recorded installation with no override must prepare the bootstrap");
    };

    let mut closed = ClosedStream;
    let result = run_bootstrapped(
        BootstrappedRun {
            prepared: &run,
            route,
            args: &[],
        },
        &mut closed,
        |_, _| {
            Ok(RunOutcome {
                code: 1,
                built_driver: true,
            })
        },
    );
    let status = result.expect("a refused notice must not fail the run");

    assert_eq!(
        status, 1,
        "the caller must receive the lint status, not a failed notice"
    );
}

/// Write the notice, on its own line, when the stream accepts it.
#[test]
fn an_accepted_notice_is_written_without_changing_the_status() {
    let mut diagnostics = Vec::new();

    let status = completed_status(
        RunOutcome {
            code: 0,
            built_driver: true,
        },
        &mut diagnostics,
    );

    assert_eq!(status, 0, "the notice must not change a successful status");
    let written = String::from_utf8(diagnostics).expect("the notice is UTF-8");
    assert!(
        written.lines().any(|line| line == BUILT_DRIVER_NOTICE),
        "a built driver must be announced on its own line: {written:?}"
    );
}

/// Say nothing about a driver when this run did not build one.
#[test]
fn a_run_that_built_nothing_writes_nothing() {
    let mut diagnostics = Vec::new();

    let status = completed_status(
        RunOutcome {
            code: 2,
            built_driver: false,
        },
        &mut diagnostics,
    );

    assert_eq!(status, 2, "an existing driver's run reports its own status");
    assert!(
        diagnostics.is_empty(),
        "a run that reused a driver has no notice to write: {:?}",
        String::from_utf8_lossy(&diagnostics)
    );
}
