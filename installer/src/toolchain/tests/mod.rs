//! Tests for toolchain detection and installation.

mod failure_mocks;
mod test_helpers;

use super::*;
use failure_mocks::{
    FailureSetup, InstallFailure, ToolchainChannel, assert_failure_error, setup_failure_mocks,
};
use rstest::rstest;
use test_helpers::{
    CapturingCommandRunner, ToolchainInstallExpectation, expect_rustc_version,
    expect_toolchain_install, matches_multi_component_add, output_with_status, output_with_stderr,
    test_toolchain,
};

const CRANELIFT_COMPONENT: &str = "rustc-codegen-cranelift";

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

fn run_missing_toolchain_install_test(extra: &[&str], expected_components: &[&str]) {
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

    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, expected_components))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    expect_rustc_version(&mut runner, &mut seq, channel, 0);

    let status = toolchain
        .ensure_installed_with(&runner, extra)
        .expect("toolchain should install");

    assert!(status.installed_toolchain());
}

#[rstest]
#[case::no_extras(
    vec![],
    REQUIRED_COMPONENTS.to_vec(),
)]
#[case::with_cranelift(
    vec![CRANELIFT_COMPONENT],
    [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat(),
)]
fn ensure_installed_installs_missing_toolchain(
    #[case] extra: Vec<&'static str>,
    #[case] expected_components: Vec<&'static str>,
) {
    run_missing_toolchain_install_test(&extra, &expected_components);
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
    let expected_components = [REQUIRED_COMPONENTS, &[CRANELIFT_COMPONENT]].concat();
    let expected_component_list = expected_components.join(", ");

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
                && components == &expected_component_list
                && message.contains("component failed")
        ),
        "expected ToolchainComponentInstallFailed with all components, got {err:?}"
    );
}

#[rstest]
#[case::toolchain_install_fails(InstallFailure::ToolchainInstall, &[])]
#[case::component_add_fails(InstallFailure::ComponentAdd, &[])]
#[case::cranelift_component_add_fails(
    InstallFailure::CraneliftComponentAdd,
    &[CRANELIFT_COMPONENT],
)]
#[case::toolchain_unusable_after_install(
    InstallFailure::ToolchainUnusableAfterInstall,
    &[],
)]
#[case::toolchain_unusable_after_install_with_cranelift(
    InstallFailure::ToolchainUnusableAfterInstall,
    &[CRANELIFT_COMPONENT],
)]
fn ensure_installed_reports_failure(
    #[case] failure: InstallFailure,
    #[case] additional_components: &[&str],
) {
    let channel = ToolchainChannel("nightly-2025-09-18");
    let toolchain = test_toolchain(channel.as_str());

    test_helpers::assert_install_fails_with(
        toolchain,
        |runner, seq| {
            setup_failure_mocks(
                runner,
                seq,
                channel,
                FailureSetup {
                    failure,
                    additional_components,
                },
            )
        },
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
