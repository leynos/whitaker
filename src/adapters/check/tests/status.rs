//! Which status a completed run reports when its notice cannot be written.
//!
//! The built-driver notice is advisory: it says a driver was built on first
//! use and later runs reuse it. It is written after the lint run has finished,
//! so a stream that refuses it must not replace the status the run produced —
//! a lint denial would otherwise be reported as an invocation that merely
//! failed to talk. ADR 0017 records that precedence, and these cases pin both
//! halves of it without starting Dylint.

use whitaker_installer::bootstrap::{BUILT_DRIVER_NOTICE, RunOutcome};

use super::{super::completed_status, fixtures::ClosedStream};

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
