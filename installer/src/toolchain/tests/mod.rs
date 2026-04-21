//! Tests for toolchain detection and installation.

mod test_helpers;

use super::*;
use rstest::rstest;
use test_helpers::{
    ToolchainInstallExpectation, assert_install_fails_with, expect_rustc_version,
    expect_toolchain_install, matches_multi_component_add, output_with_status, output_with_stderr,
    test_toolchain,
};

const CRANELIFT_COMPONENT: &str = "rustc-codegen-cranelift";

/// A typed toolchain channel identifier for use in tests
/// (e.g. `"nightly-2025-09-18"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ToolchainChannel<'a>(&'a str);

impl<'a> ToolchainChannel<'a> {
    fn as_str(self) -> &'a str {
        self.0
    }
}

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

#[rstest]
#[case::missing_channel("[toolchain]\ncomponents = [\"rust-src\"]\n", "channel")]
#[case::invalid_toml("this is not valid toml {{{", "TOML")]
fn rejects_invalid_toolchain_file(#[case] contents: &str, #[case] expected_reason: &str) {
    assert_parse_fails_with_reason(contents, expected_reason, parse_toolchain_channel);
}

#[test]
fn ensure_installed_installs_missing_toolchain() {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel);

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

    // Expect required components to be installed
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, REQUIRED_COMPONENTS))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    expect_rustc_version(&mut runner, &mut seq, channel, 0);

    let status = toolchain
        .ensure_installed_with(&runner, &[])
        .expect("toolchain should install");

    assert!(status.installed_toolchain());
}

fn run_component_installation_test(extra: &[&str], expected: &[&str]) {
    let channel = "nightly-2025-09-18";
    let toolchain = test_toolchain(channel);
    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, channel, 0);
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, expected))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    let status = toolchain
        .ensure_installed_with(&runner, extra)
        .expect("toolchain should be ready");

    assert!(!status.installed_toolchain());
}

#[rstest]
#[case::no_extras(vec![], REQUIRED_COMPONENTS.to_vec())]
#[case::with_cranelift(
    vec![CRANELIFT_COMPONENT],
    [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat()
)]
fn ensure_installed_adds_correct_components(
    #[case] extra: Vec<&'static str>,
    #[case] expected: Vec<&'static str>,
) {
    run_component_installation_test(&extra, &expected);
}

// Describes the type of installation failure being tested.
#[derive(Debug, Clone, Copy)]
enum InstallFailure {
    ToolchainInstall,
    ComponentAdd,
    CraneliftComponentAdd,
    ToolchainUnusableAfterInstall,
}

fn setup_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: ToolchainChannel<'_>,
    failure: InstallFailure,
) {
    let channel = channel.as_str();
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
            runner
                .expect_run()
                .withf(|program, args| {
                    program == "rustup"
                        && args.len() >= 4
                        && args[0] == "component"
                        && args[1] == "add"
                })
                .times(1)
                .in_sequence(seq)
                .returning(|_, _| Ok(output_with_stderr(1, "component failed")));
        }
        InstallFailure::CraneliftComponentAdd => {
            let expected_components = [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat();
            expect_rustc_version(runner, seq, channel, 0);
            runner
                .expect_run()
                .withf(matches_multi_component_add(channel, &expected_components))
                .times(1)
                .in_sequence(seq)
                .returning(|_, _| Ok(output_with_stderr(1, "component failed")));
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
            runner
                .expect_run()
                .withf(|program, args| {
                    program == "rustup"
                        && args.len() >= 4
                        && args[0] == "component"
                        && args[1] == "add"
                })
                .times(1)
                .in_sequence(seq)
                .returning(|_, _| Ok(output_with_status(0)));
            expect_rustc_version(runner, seq, channel, 1);
        }
    }
}

fn assert_toolchain_install_failed(err: InstallerError, channel: ToolchainChannel<'_>) {
    let channel = channel.as_str();
    assert!(
        matches!(
            err,
            InstallerError::ToolchainInstallFailed { ref toolchain, ref message }
                if toolchain == channel && message.contains("network down")
        ),
        "expected ToolchainInstallFailed error, got {err:?}"
    );
}

fn assert_component_add_failed(err: InstallerError, channel: ToolchainChannel<'_>) {
    let channel = channel.as_str();
    assert!(
        matches!(
            err,
            InstallerError::ToolchainComponentInstallFailed {
                ref toolchain,
                ref message,
                ..
            } if toolchain == channel && message.contains("component failed")
        ),
        "expected ToolchainComponentInstallFailed error, got {err:?}"
    );
}

fn assert_cranelift_component_add_failed(err: InstallerError, channel: ToolchainChannel<'_>) {
    let channel = channel.as_str();
    assert!(
        matches!(
            err,
            InstallerError::ToolchainComponentInstallFailed {
                ref toolchain,
                ref components,
                ref message,
            } if toolchain == channel
                && components.contains(CRANELIFT_COMPONENT)
                && message.contains("component failed")
        ),
        "expected ToolchainComponentInstallFailed with cranelift component, got {err:?}"
    );
}

fn assert_toolchain_not_installed(err: InstallerError, channel: ToolchainChannel<'_>) {
    let channel = channel.as_str();
    assert!(
        matches!(
            err,
            InstallerError::ToolchainNotInstalled { ref toolchain }
                if toolchain == channel
        ),
        "expected ToolchainNotInstalled error, got {err:?}"
    );
}

fn assert_failure_error(err: InstallerError, channel: ToolchainChannel<'_>, failure: InstallFailure) {
    let channel = channel.as_str();
    match failure {
        InstallFailure::ToolchainInstall => {
            assert_toolchain_install_failed(err, ToolchainChannel(channel))
        }
        InstallFailure::ComponentAdd => {
            assert_component_add_failed(err, ToolchainChannel(channel))
        }
        InstallFailure::CraneliftComponentAdd => {
            assert_cranelift_component_add_failed(err, ToolchainChannel(channel))
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            assert_toolchain_not_installed(err, ToolchainChannel(channel))
        }
    }
}

#[rstest]
#[case::toolchain_install_fails(InstallFailure::ToolchainInstall)]
#[case::component_add_fails(InstallFailure::ComponentAdd)]
#[case::cranelift_component_add_fails(InstallFailure::CraneliftComponentAdd)]
#[case::toolchain_unusable_after_install(InstallFailure::ToolchainUnusableAfterInstall)]
fn ensure_installed_reports_failure(#[case] failure: InstallFailure) {
    let channel = ToolchainChannel("nightly-2025-09-18");
    let toolchain = test_toolchain(channel.as_str());
    let additional_components = match failure {
        InstallFailure::CraneliftComponentAdd => &[CRANELIFT_COMPONENT][..],
        _ => &[],
    };

    assert_install_fails_with(
        toolchain,
        |runner, seq| setup_failure_mocks(runner, seq, channel, failure),
        |toolchain, runner| toolchain.ensure_installed_with(runner, additional_components),
        |err| assert_failure_error(err, channel, failure),
    );
}

#[rstest]
#[case::empty_stderr(None, "unknown error")]
#[case::whitespace_only(Some("  \n\t  "), "unknown error")]
#[case::trailing_whitespace(Some("some error message   \n\n"), "some error message")]
#[case::multiline_utf8(Some("line one\nline two\n"), "line one\nline two")]
fn stderr_message_extracts_error(#[case] stderr: Option<&str>, #[case] expected: &str) {
    let output = match stderr {
        Some(s) => output_with_stderr(1, s),
        None => output_with_status(1),
    };
    assert_eq!(stderr_message(&output), expected);
}

#[test]
fn parses_toolchain_file_ignoring_components() {
    // The installer ignores components from rust-toolchain.toml and uses
    // REQUIRED_COMPONENTS instead. This test verifies parsing succeeds even
    // with components present in the file.
    let contents = r#"
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
"#;
    let config = parse_toolchain_config(contents).expect("config should parse");
    assert_eq!(config.channel, "stable");
}

#[test]
fn run_rustup_propagates_io_error_as_toolchain_detection_error() {
    use std::io;

    let mut runner = MockCommandRunner::new();

    runner
        .expect_run()
        .returning(|_, _| Err(io::Error::other("boom")));

    let result = run_rustup(&runner, &["toolchain", "list"]);

    assert!(
        matches!(result, Err(InstallerError::ToolchainDetection { .. })),
        "expected ToolchainDetection error, got {result:?}"
    );
}
