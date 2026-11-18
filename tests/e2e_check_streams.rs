//! End-to-end assertions that `whitaker check` keeps standard output pure.
//!
//! During `check`, every byte on standard output must come from the Dylint
//! child, because a caller may pipe it into a JSON consumer. Whitaker's own
//! notices — including the built-driver notice and the repair notice — belong
//! on standard error. This runs the real binary against a stubbed child that
//! prints one JSON line, which is the shape `cargo --message-format=json`
//! produces.

#![cfg(unix)]

use std::process::Output;

// The shared fixture is written for the root-CLI behaviour suites; this one
// uses only its installed bootstrap and command builder.
#[expect(
    dead_code,
    reason = "the shared fixture also serves the root-CLI behaviour suites"
)]
#[path = "behaviour_whitaker/support.rs"]
mod support;

/// The single JSON line the stub child prints.
const JSON_LINE: &str = r#"{"reason":"build-finished","success":true}"#;

/// Run `check` against a provisioned fixture whose stub prints `JSON_LINE`.
fn run_check(fixture: &support::BootstrapFixture) -> std::io::Result<Output> {
    let mut command = fixture.command().map_err(std::io::Error::other)?;
    command
        .args(["check", "--", "--message-format=json"])
        .env("STUB_STDOUT", JSON_LINE)
        .output()
}

#[test]
fn machine_readable_child_output_is_not_polluted() {
    let fixture =
        support::BootstrapFixture::new().expect("the fixture installation should be created");

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

/// The first run builds the driver, and that notice is Whitaker's own. It
/// must reach standard error, so this test fails if the purity assertion above
/// only passes because no notice was produced.
#[test]
fn whitakers_own_notice_reaches_standard_error() {
    let fixture =
        support::BootstrapFixture::new().expect("the fixture installation should be created");

    let output = run_check(&fixture).expect("whitaker check process should start");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("driver"),
        "the built-driver notice should explain the first run on stderr: {stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("driver"),
        "no Whitaker notice may reach standard output"
    );
}
