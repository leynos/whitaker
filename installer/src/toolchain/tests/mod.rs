//! Tests for toolchain detection and installation.

mod test_helpers;

use super::*;
use rstest::rstest;
use test_helpers::{
    ComponentAddExpectation, ToolchainInstallExpectation, assert_install_fails_with,
    expect_component_add, expect_rustc_version, expect_toolchain_install, output_with_status,
    output_with_stderr, test_toolchain,
};

// Asserts that a parsing function rejects invalid contents with an
// InvalidToolchainFile error containing the expected reason substring.
fn assert_parse_fails_with_reason<F, T>(contents: &str, expected_reason: &str, parse_fn: F)
where
    F: FnOnce(&str) -> Result<T>,
    T: std::fmt::Debug,
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

// Identifies which parse function to use in parameterized tests.
#[derive(Debug, Clone, Copy)]
enum ParseTarget {
    Channel,
    Config,
}

fn run_parse_and_check(contents: &str, expected_reason: &str, target: ParseTarget) {
    match target {
        ParseTarget::Channel => {
            assert_parse_fails_with_reason(contents, expected_reason, parse_toolchain_channel)
        }
        ParseTarget::Config => {
            assert_parse_fails_with_reason(contents, expected_reason, parse_toolchain_config)
        }
    }
}

#[rstest]
#[case::missing_channel(
    ParseTarget::Channel,
    "[toolchain]\ncomponents = [\"rust-src\"]\n",
    "channel"
)]
#[case::invalid_toml(ParseTarget::Channel, "this is not valid toml {{{", "TOML")]
#[case::invalid_components_type(
    ParseTarget::Config,
    "[toolchain]\nchannel = \"nightly-2025-09-18\"\ncomponents = \"rust-src\"\n",
    "array"
)]
#[case::non_string_component_elements(
    ParseTarget::Config,
    "[toolchain]\nchannel = \"stable\"\ncomponents = [123, \"rust-src\"]\n",
    "array of strings"
)]
fn rejects_invalid_toolchain_file(
    #[case] target: ParseTarget,
    #[case] contents: &str,
    #[case] expected_reason: &str,
) {
    run_parse_and_check(contents, expected_reason, target);
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

// Describes the type of installation failure being tested.
#[derive(Debug, Clone, Copy)]
enum InstallFailure {
    ToolchainInstall,
    ComponentAdd,
    ToolchainUnusableAfterInstall,
}

fn setup_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    failure: InstallFailure,
) {
    match failure {
        InstallFailure::ToolchainInstall => {
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
        }
        InstallFailure::ComponentAdd => {
            expect_rustc_version(runner, seq, channel, 0);
            expect_component_add(
                runner,
                seq,
                ComponentAddExpectation {
                    channel,
                    component: "rust-src",
                    exit_code: 1,
                    stderr: Some("component failed"),
                },
            );
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            expect_rustc_version(runner, seq, channel, 1);
            expect_toolchain_install(
                runner,
                seq,
                ToolchainInstallExpectation {
                    channel,
                    exit_code: 0,
                    stderr: None,
                },
            );
            expect_rustc_version(runner, seq, channel, 1);
        }
    }
}

fn assert_failure_error(err: InstallerError, channel: &str, failure: InstallFailure) {
    match failure {
        InstallFailure::ToolchainInstall => {
            assert!(
                matches!(
                    err,
                    InstallerError::ToolchainInstallFailed { ref toolchain, ref message }
                        if toolchain == channel && message.contains("network down")
                ),
                "expected ToolchainInstallFailed error, got {err:?}"
            );
        }
        InstallFailure::ComponentAdd => {
            assert!(
                matches!(
                    err,
                    InstallerError::ToolchainComponentInstallFailed { ref toolchain, ref message, .. }
                        if toolchain == channel && message.contains("component failed")
                ),
                "expected ToolchainComponentInstallFailed error, got {err:?}"
            );
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            assert!(
                matches!(
                    err,
                    InstallerError::ToolchainNotInstalled { ref toolchain }
                        if toolchain == channel
                ),
                "expected ToolchainNotInstalled error, got {err:?}"
            );
        }
    }
}

#[rstest]
#[case::toolchain_install_fails(InstallFailure::ToolchainInstall, Vec::new())]
#[case::component_add_fails(InstallFailure::ComponentAdd, vec!["rust-src".to_owned()])]
#[case::toolchain_unusable_after_install(InstallFailure::ToolchainUnusableAfterInstall, Vec::new())]
fn ensure_installed_reports_failure(
    #[case] failure: InstallFailure,
    #[case] components: Vec<String>,
) {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel, components);

    assert_install_fails_with(
        toolchain,
        |runner, seq| setup_failure_mocks(runner, seq, channel, failure),
        |err| assert_failure_error(err, channel, failure),
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
