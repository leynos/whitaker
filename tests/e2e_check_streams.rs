//! End-to-end assertions that `whitaker check` keeps standard output pure.
//!
//! During `check`, every byte on standard output must come from the Dylint
//! child, because a caller may pipe it into a JSON consumer. Whitaker's own
//! notices — including the built-driver notice — belong on standard error.
//! These tests run the real binary against a stubbed child that prints one
//! JSON line, which is the shape `cargo --message-format=json` produces.

#![cfg(unix)]

use std::process::Output;

use whitaker_installer::bootstrap::BUILT_DRIVER_NOTICE;

mod e2e_check_support;

/// The single JSON line the stub child prints.
const JSON_LINE: &str = r#"{"reason":"build-finished","success":true}"#;

/// Run `check` against a provisioned fixture whose stub prints `JSON_LINE`.
fn run_check(fixture: &e2e_check_support::BootstrapFixture) -> std::io::Result<Output> {
    let mut command = fixture.command().map_err(std::io::Error::other)?;
    command
        .args(["check", "--", "--message-format=json"])
        .env("STUB_STDOUT", JSON_LINE)
        .output()
}

/// Verify that a machine-readable child's standard output is not polluted.
#[test]
fn machine_readable_child_output_is_not_polluted() {
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");

    let output = run_check(&fixture).expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(23),
        "the child's status must pass through, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{JSON_LINE}\n"),
        "standard output must hold the child's line and nothing else"
    );
}

/// Verify that a cold check writes the built-driver notice to standard error.
///
/// The first run builds the driver, so the notice is Whitaker's own and must
/// reach stderr as a complete line. This test fails if the purity assertion
/// above only passes because no notice was produced; the status and standard
/// output are rechecked here so the notice and the cold path are proven on the
/// same invocation.
#[test]
fn whitakers_own_notice_reaches_standard_error() {
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");

    let output = run_check(&fixture).expect("whitaker check process should start");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(23),
        "the child's status must pass through, stderr: {stderr}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{JSON_LINE}\n"),
        "standard output must hold the child's line only, stderr: {stderr}"
    );
    assert!(
        stderr.lines().any(|line| line == BUILT_DRIVER_NOTICE),
        "the built-driver notice should be its own stderr line, got: {stderr}"
    );
}
