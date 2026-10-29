//! `scripts/verify-installed-bootstrap.ps1` fails when any property breaks.
//!
//! The script runs only after a release publishes, so this runs the real
//! script under `pwsh` against `PATH` stubs (see `windows_verify_support`),
//! with `-HostsFile` redirecting the github.com denial into a scratch file so
//! the runner's real hosts file is never altered. It mirrors the bash
//! scenarios in `tests/workflow_contracts/verify_installed_bootstrap_test.py`.
#![cfg(windows)]

mod windows_verify_support;

use rstest::rstest;
use std::fs;
use std::process::Output;
use windows_verify_support::Sandbox;

/// Return the script's standard error as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_healthy_installation_passes() {
    let sandbox = Sandbox::default();
    let output = sandbox.run(&[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let hosts = fs::read_to_string(sandbox.hosts()).expect("read hosts");
    assert!(hosts.contains("0.0.0.0 github.com"), "{hosts}");
    assert!(sandbox.snapshot().is_dir(), "snapshot not restored");
}

#[rstest]
#[case::denial_ineffective("FAKE_DENIAL_INEFFECTIVE", "canary still succeeds")]
#[case::lint_passes("FAKE_SUCCEED", "succeeded on a std::fs violation")]
#[case::crash_not_diagnostic("FAKE_NO_DIAGNOSTIC", "without a no_std_fs_operations diagnostic")]
#[case::warm_rebuilds("FAKE_REBUILD_ALWAYS", "the warm run rebuilt the driver")]
#[case::concurrent_fails("FAKE_CONCURRENT_FAILS", "concurrent-")]
#[case::runs_without_snapshot("FAKE_SUCCEED_WITHOUT_SNAPSHOT", "ran without its Clippy snapshot")]
#[case::no_fallback_message_missing(
    "FAKE_SILENT_WITHOUT_SNAPSHOT",
    "missing snapshot did not fail clearly"
)]
#[case::lints_despite_missing_snapshot(
    "FAKE_LINT_WITHOUT_SNAPSHOT",
    "lints ran although the snapshot was missing"
)]
fn a_violated_property_fails_the_script(#[case] toggle: &str, #[case] message: &str) {
    let sandbox = Sandbox::default();
    let output = sandbox.run(&[(toggle, "1")]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(stderr(&output).contains(message), "{}", stderr(&output));
}

#[test]
fn a_missing_snapshot_fails_before_any_lint_runs() {
    let sandbox = Sandbox::default();
    fs::remove_dir(sandbox.snapshot()).expect("remove the snapshot");
    let output = sandbox.run(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("is missing"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_snapshot_is_restored_after_a_failing_deleted_snapshot_check() {
    let sandbox = Sandbox::default();
    sandbox.run(&[("FAKE_SUCCEED_WITHOUT_SNAPSHOT", "1")]);
    assert!(sandbox.snapshot().is_dir());
}
