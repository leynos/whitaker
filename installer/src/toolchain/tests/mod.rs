//! Tests for toolchain detection and installation.

mod test_helpers;

use super::*;
use rstest::rstest;
use std::cell::RefCell;
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

#[test]
fn ensure_installed_adds_required_and_additional_components_when_toolchain_missing() {
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

    let expected_components = [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat();
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, &expected_components))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    expect_rustc_version(&mut runner, &mut seq, channel, 0);

    let status = toolchain
        .ensure_installed_with(&runner, &[CRANELIFT_COMPONENT])
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

struct CapturingCommandRunner {
    calls: RefCell<Vec<(String, Vec<String>)>>,
    output: Output,
}

impl CapturingCommandRunner {
    fn new(output: Output) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            output,
        }
    }

    fn recorded_calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for CapturingCommandRunner {
    fn run<'a>(&self, program: &str, args: &[&'a str]) -> std::io::Result<Output> {
        self.calls.borrow_mut().push((
            program.to_owned(),
            args.iter().map(|arg| (*arg).to_owned()).collect(),
        ));
        Ok(self.output.clone())
    }
}

#[test]
fn install_components_with_additional_components_assembles_rustup_args_in_order() {
    let toolchain = test_toolchain("nightly-2025-09-18");
    let runner = CapturingCommandRunner::new(output_with_status(0));

    toolchain
        .install_components_with(&runner, &[CRANELIFT_COMPONENT])
        .expect("component installation should succeed");

    let expected_args: Vec<String> = ["component", "add", "--toolchain", "nightly-2025-09-18"]
        .into_iter()
        .chain(REQUIRED_COMPONENTS.iter().copied())
        .chain([CRANELIFT_COMPONENT])
        .map(str::to_owned)
        .collect();
    assert_eq!(
        runner.recorded_calls(),
        vec![("rustup".to_owned(), expected_args)]
    );
}

#[test]
fn install_components_with_failure_reports_all_components() {
    let toolchain = test_toolchain("nightly-2025-09-18");
    let runner = CapturingCommandRunner::new(output_with_stderr(1, "component failed"));

    let err = toolchain
        .install_components_with(&runner, &[CRANELIFT_COMPONENT])
        .expect_err("component installation should fail");

    assert!(
        matches!(
            err,
            InstallerError::ToolchainComponentInstallFailed {
                ref toolchain,
                ref components,
                ref message,
            } if toolchain == "nightly-2025-09-18"
                && components == "rust-src, rustc-dev, llvm-tools-preview, rustc-codegen-cranelift"
                && message.contains("component failed")
        ),
        "expected ToolchainComponentInstallFailed with all components, got {err:?}"
    );
}

// Describes the type of installation failure being tested.
#[derive(Debug, Clone, Copy)]
enum InstallFailure {
    ToolchainInstall,
    ComponentAdd,
    CraneliftComponentAdd,
    ToolchainUnusableAfterInstall,
}

fn setup_toolchain_install_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
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

fn setup_component_add_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    expect_rustc_version(runner, seq, channel, 0);
    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup" && args.len() >= 4 && args[0] == "component" && args[1] == "add"
        })
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_stderr(1, "component failed")));
}

fn setup_cranelift_component_add_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    let expected_components = [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat();
    expect_rustc_version(runner, seq, channel, 0);
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, &expected_components))
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_stderr(1, "component failed")));
}

fn setup_toolchain_unusable_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
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
            program == "rustup" && args.len() >= 4 && args[0] == "component" && args[1] == "add"
        })
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_status(0)));
    expect_rustc_version(runner, seq, channel, 1);
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
            setup_toolchain_install_failure_mocks(runner, seq, channel);
        }
        InstallFailure::ComponentAdd => {
            setup_component_add_failure_mocks(runner, seq, channel);
        }
        InstallFailure::CraneliftComponentAdd => {
            setup_cranelift_component_add_failure_mocks(runner, seq, channel);
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            setup_toolchain_unusable_failure_mocks(runner, seq, channel);
        }
    }
}

/// Asserts that `err` satisfies `predicate`, printing `description` on failure.
fn assert_error_matches<F>(err: &InstallerError, description: &str, predicate: F)
where
    F: FnOnce(&InstallerError) -> bool,
{
    assert!(predicate(err), "expected {description}, got {err:?}");
}

fn is_cranelift_component_install_failed(err: &InstallerError, channel: &str) -> bool {
    let InstallerError::ToolchainComponentInstallFailed {
        toolchain,
        components,
        message,
    } = err
    else {
        return false;
    };
    if toolchain != channel {
        return false;
    }
    if !components.contains(CRANELIFT_COMPONENT) {
        return false;
    }
    message.contains("component failed")
}

fn assert_failure_error(err: InstallerError, channel: &str, failure: InstallFailure) {
    match failure {
        InstallFailure::ToolchainInstall => assert_error_matches(
            &err,
            &format!("ToolchainInstallFailed for {channel}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainInstallFailed { toolchain, message }
                        if toolchain == channel && message.contains("network down")
                )
            },
        ),
        InstallFailure::ComponentAdd => assert_error_matches(
            &err,
            &format!("ToolchainComponentInstallFailed for {channel}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainComponentInstallFailed {
                        toolchain,
                        message,
                        ..
                    } if toolchain == channel && message.contains("component failed")
                )
            },
        ),
        InstallFailure::CraneliftComponentAdd => assert_error_matches(
            &err,
            &format!("ToolchainComponentInstallFailed with cranelift for {channel}"),
            |e| is_cranelift_component_install_failed(e, channel),
        ),
        InstallFailure::ToolchainUnusableAfterInstall => {
            assert_error_matches(&err, &format!("ToolchainNotInstalled for {channel}"), |e| {
                matches!(
                    e,
                    InstallerError::ToolchainNotInstalled { toolchain }
                        if toolchain == channel
                )
            })
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
        |err| assert_failure_error(err, channel.as_str(), failure),
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
