//! Tests for toolchain detection and installation.

use super::*;
use std::process::ExitStatus;

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatusExt::from_raw(code << 8)
}

#[cfg(windows)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatusExt::from_raw(code as u32)
}

fn output_with_status(code: i32) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn output_with_stderr(code: i32, stderr: &str) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

fn test_toolchain(channel: &str, components: Vec<String>) -> Toolchain {
    Toolchain {
        channel: channel.to_owned(),
        components,
        workspace_root: Utf8PathBuf::from("."),
    }
}

struct ToolchainInstallExpectation<'a> {
    channel: &'a str,
    exit_code: i32,
    stderr: Option<&'a str>,
}

struct ComponentAddExpectation<'a> {
    channel: &'a str,
    component: &'a str,
    exit_code: i32,
    stderr: Option<&'a str>,
}

fn expect_rustc_version(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    exit_code: i32,
) {
    let channel = channel.to_owned();
    runner
        .expect_run()
        .withf(move |program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == channel
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(seq)
        .returning(move |_, _| Ok(output_with_status(exit_code)));
}

fn expect_toolchain_install(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    expectation: ToolchainInstallExpectation<'_>,
) {
    let channel = expectation.channel.to_owned();
    let stderr = expectation.stderr.map(str::to_owned);
    let exit_code = expectation.exit_code;
    runner
        .expect_run()
        .withf(move |program, args| {
            program == "rustup"
                && args.len() == 3
                && args[0] == "toolchain"
                && args[1] == "install"
                && args[2] == channel
        })
        .times(1)
        .in_sequence(seq)
        .returning(move |_, _| {
            let output = match stderr.as_deref() {
                Some(message) => output_with_stderr(exit_code, message),
                None => output_with_status(exit_code),
            };
            Ok(output)
        });
}

fn expect_component_add(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    expectation: ComponentAddExpectation<'_>,
) {
    let channel = expectation.channel.to_owned();
    let component = expectation.component.to_owned();
    let stderr = expectation.stderr.map(str::to_owned);
    let exit_code = expectation.exit_code;
    runner
        .expect_run()
        .withf(move |program, args| {
            program == "rustup"
                && args.len() == 5
                && args[0] == "component"
                && args[1] == "add"
                && args[2] == "--toolchain"
                && args[3] == channel
                && args[4] == component
        })
        .times(1)
        .in_sequence(seq)
        .returning(move |_, _| {
            let output = match stderr.as_deref() {
                Some(message) => output_with_stderr(exit_code, message),
                None => output_with_status(exit_code),
            };
            Ok(output)
        });
}

/// Helper to test that ensure_installed fails with the expected error.
fn assert_install_fails_with<F, E>(toolchain: Toolchain, setup_mocks: F, error_matcher: E)
where
    F: FnOnce(&mut MockCommandRunner, &mut mockall::Sequence),
    E: FnOnce(InstallerError),
{
    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    setup_mocks(&mut runner, &mut seq);

    let err = toolchain
        .ensure_installed_with(&runner)
        .expect_err("expected installation failure");

    error_matcher(err);
}

#[test]
fn parses_standard_toolchain_format() {
    let contents = r#"
[toolchain]
channel = "nightly-2025-09-18"
components = ["rust-src", "rustc-dev"]
"#;
    let channel = parse_toolchain_channel(contents);
    assert!(channel.is_ok());
    assert_eq!(channel.ok(), Some("nightly-2025-09-18".to_owned()));
}

#[test]
fn parses_simple_channel_format() {
    let contents = r#"channel = "stable""#;
    let channel = parse_toolchain_channel(contents);
    assert!(channel.is_ok());
    assert_eq!(channel.ok(), Some("stable".to_owned()));
}

#[test]
fn rejects_missing_channel() {
    let contents = r#"
[toolchain]
components = ["rust-src"]
"#;
    let result = parse_toolchain_channel(contents);
    assert!(result.is_err());
}

#[test]
fn rejects_invalid_toml() {
    let contents = "this is not valid toml {{{";
    let result = parse_toolchain_channel(contents);
    assert!(result.is_err());
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
    let result = parse_toolchain_config(contents);
    assert!(result.is_err());
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
    let result = parse_toolchain_config(contents);
    assert!(result.is_err());
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
