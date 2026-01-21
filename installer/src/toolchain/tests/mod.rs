//! Tests for toolchain detection and installation.

mod test_helpers;

use super::*;
use test_helpers::{
    ComponentAddExpectation, ToolchainInstallExpectation, assert_install_fails_with,
    expect_component_add, expect_rustc_version, expect_toolchain_install, output_with_status,
    output_with_stderr, test_toolchain,
};

/// Helper to assert that a parsing function rejects invalid contents
/// with an InvalidToolchainFile error containing the expected reason substring.
fn assert_parse_fails_with_reason<F>(contents: &str, expected_reason: &str, parse_fn: F)
where
    F: FnOnce(&str) -> Result<()>,
{
    let err = parse_fn(contents).expect_err("should reject invalid toolchain file");
    assert!(
        matches!(
            err,
            InstallerError::InvalidToolchainFile { ref reason }
                if reason.contains(expected_reason)
        ),
        "expected InvalidToolchainFile error containing '{expected_reason}', got {err:?}"
    );
}

#[test]
fn parses_standard_toolchain_format() {
    let contents = r#"
[toolchain]
channel = "nightly-2025-09-18"
components = ["rust-src", "rustc-dev"]
"#;
    let channel =
        parse_toolchain_channel(contents).expect("should parse standard toolchain format");
    assert_eq!(channel, "nightly-2025-09-18");
}

#[test]
fn parses_simple_channel_format() {
    let contents = r#"channel = "stable""#;
    let channel = parse_toolchain_channel(contents).expect("should parse simple channel format");
    assert_eq!(channel, "stable");
}

#[test]
fn rejects_missing_channel() {
    let contents = r#"
[toolchain]
components = ["rust-src"]
"#;
    assert_parse_fails_with_reason(contents, "channel", |c| {
        parse_toolchain_channel(c).map(|_| ())
    });
}

#[test]
fn rejects_invalid_toml() {
    let contents = "this is not valid toml {{{";
    assert_parse_fails_with_reason(contents, "TOML", |c| parse_toolchain_channel(c).map(|_| ()));
}

#[test]
fn parses_components_from_toolchain_table() {
    let contents = r#"
[toolchain]
channel = "nightly-2025-09-18"
components = ["rust-src", "rustc-dev"]
"#;
    let config = parse_toolchain_config(contents).expect("config should parse");
    assert_eq!(
        config.components,
        vec!["rust-src".to_owned(), "rustc-dev".to_owned()]
    );
}

#[test]
fn rejects_invalid_components() {
    let contents = r#"
[toolchain]
channel = "nightly-2025-09-18"
components = "rust-src"
"#;
    assert_parse_fails_with_reason(contents, "array", |c| parse_toolchain_config(c).map(|_| ()));
}

#[test]
fn ensure_installed_installs_missing_toolchain() {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel, Vec::new());

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, channel, 1);
    expect_toolchain_install(
        &mut runner,
        &mut seq,
        ToolchainInstallExpectation {
            channel,
            exit_code: 0,
            stderr: None,
        },
    );
    expect_rustc_version(&mut runner, &mut seq, channel, 0);

    let status = toolchain
        .ensure_installed_with(&runner)
        .expect("toolchain should install");

    assert!(status.installed_toolchain());
}

#[test]
fn ensure_installed_adds_components_when_present() {
    let toolchain = test_toolchain("nightly-2025-09-18", vec!["rust-src".to_owned()]);
    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, "nightly-2025-09-18", 0);
    expect_component_add(
        &mut runner,
        &mut seq,
        ComponentAddExpectation {
            channel: "nightly-2025-09-18",
            component: "rust-src",
            exit_code: 0,
            stderr: None,
        },
    );

    let status = toolchain
        .ensure_installed_with(&runner)
        .expect("toolchain should be ready");

    assert!(!status.installed_toolchain());
}

#[test]
fn ensure_installed_reports_toolchain_install_failure() {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel, Vec::new());

    assert_install_fails_with(
        toolchain,
        |runner, seq| {
            expect_rustc_version(runner, seq, channel, 1);
            expect_toolchain_install(
                runner,
                seq,
                ToolchainInstallExpectation {
                    channel,
                    exit_code: 1,
                    stderr: Some("network down"),
                },
            );
        },
        |err| {
            assert!(
                matches!(
                    err,
                    InstallerError::ToolchainInstallFailed { ref toolchain, ref message }
                        if toolchain == "nightly-2025-09-18" && message.contains("network down")
                ),
                "unexpected error: {err}"
            );
        },
    );
}

#[test]
fn ensure_installed_reports_component_install_failure() {
    let channel = "nightly-2025-09-18";
    let component = "rust-src";
    let toolchain = test_toolchain(channel, vec![component.to_owned()]);

    assert_install_fails_with(
        toolchain,
        |runner, seq| {
            expect_rustc_version(runner, seq, channel, 0);
            expect_component_add(
                runner,
                seq,
                ComponentAddExpectation {
                    channel,
                    component,
                    exit_code: 1,
                    stderr: Some("component failed"),
                },
            );
        },
        |err| {
            assert!(
                matches!(
                    err,
                    InstallerError::ToolchainComponentInstallFailed { ref toolchain, ref message, .. }
                        if toolchain == "nightly-2025-09-18" && message.contains("component failed")
                ),
                "unexpected error: {err}"
            );
        },
    );
}

#[test]
fn stderr_message_empty_stderr_returns_unknown_error() {
    let output = output_with_status(1);
    assert_eq!(stderr_message(&output), "unknown error");
}

#[test]
fn stderr_message_whitespace_only_returns_unknown_error() {
    let output = output_with_stderr(1, "  \n\t  ");
    assert_eq!(stderr_message(&output), "unknown error");
}

#[test]
fn stderr_message_trims_trailing_whitespace() {
    let output = output_with_stderr(1, "some error message   \n\n");
    assert_eq!(stderr_message(&output), "some error message");
}

#[test]
fn stderr_message_handles_multiline_utf8() {
    let output = output_with_stderr(1, "line one\nline two\n");
    assert_eq!(stderr_message(&output), "line one\nline two");
}

#[test]
fn parses_top_level_components() {
    let contents = r#"
channel = "stable"
components = ["rust-src"]
"#;
    let config = parse_toolchain_config(contents).expect("config should parse");
    assert_eq!(config.components, vec!["rust-src".to_owned()]);
}

#[test]
fn parses_empty_components_array() {
    let contents = r#"
[toolchain]
channel = "stable"
components = []
"#;
    let config = parse_toolchain_config(contents).expect("config should parse");
    assert!(config.components.is_empty());
}

#[test]
fn parses_missing_components_as_empty() {
    let contents = r#"
[toolchain]
channel = "stable"
"#;
    let config = parse_toolchain_config(contents).expect("config should parse");
    assert!(config.components.is_empty());
}

#[test]
fn rejects_non_string_component_elements() {
    let contents = r#"
[toolchain]
channel = "stable"
components = [123, "rust-src"]
"#;
    assert_parse_fails_with_reason(contents, "array of strings", |c| {
        parse_toolchain_config(c).map(|_| ())
    });
}

#[test]
fn run_rustup_propagates_io_error_as_toolchain_detection_error() {
    use std::io;

    let mut runner = MockCommandRunner::new();

    runner
        .expect_run()
        .returning(|_, _| Err(io::Error::other("boom")));

    let args = vec!["toolchain".to_owned(), "list".to_owned()];
    let result = run_rustup(&runner, &args);

    assert!(
        matches!(result, Err(InstallerError::ToolchainDetection { .. })),
        "expected ToolchainDetection error, got {result:?}"
    );
}

#[test]
fn ensure_installed_fails_when_toolchain_unusable_after_install() {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel, Vec::new());

    assert_install_fails_with(
        toolchain,
        |runner, seq| {
            // First rustc --version fails -> triggers installation
            expect_rustc_version(runner, seq, channel, 1);
            // Installation succeeds
            expect_toolchain_install(
                runner,
                seq,
                ToolchainInstallExpectation {
                    channel,
                    exit_code: 0,
                    stderr: None,
                },
            );
            // Second rustc --version still fails -> toolchain unusable
            expect_rustc_version(runner, seq, channel, 1);
        },
        |err| {
            assert!(
                matches!(err, InstallerError::ToolchainNotInstalled { ref toolchain }
                    if toolchain == "nightly-2025-09-18"),
                "expected ToolchainNotInstalled error, got {err:?}"
            );
        },
    );
}

#[test]
fn ensure_installed_adds_multiple_components() {
    let components = vec!["rust-src".to_owned(), "rustc-dev".to_owned()];
    let toolchain = test_toolchain("nightly-2025-09-18", components.clone());
    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, "nightly-2025-09-18", 0);

    // Expect a single component add call with both components
    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 6
                && args[0] == "component"
                && args[1] == "add"
                && args[2] == "--toolchain"
                && args[3] == "nightly-2025-09-18"
                && args[4] == "rust-src"
                && args[5] == "rustc-dev"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    let status = toolchain
        .ensure_installed_with(&runner)
        .expect("toolchain should be ready");

    assert!(!status.installed_toolchain());
}
