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

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, channel, 1);
    expect_toolchain_install(
        &mut runner,
        &mut seq,
        ToolchainInstallExpectation {
            channel,
            exit_code: 1,
            stderr: Some("network down"),
        },
    );

    let err = toolchain
        .ensure_installed_with(&runner)
        .expect_err("expected toolchain install failure");

    assert!(
        matches!(
            err,
            InstallerError::ToolchainInstallFailed { ref toolchain, ref message }
                if toolchain == "nightly-2025-09-18" && message.contains("network down")
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn ensure_installed_reports_component_install_failure() {
    let channel = "nightly-2025-09-18";
    let component = "rust-src";
    let toolchain = test_toolchain(channel, vec![component.to_owned()]);

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    expect_rustc_version(&mut runner, &mut seq, channel, 0);
    expect_component_add(
        &mut runner,
        &mut seq,
        ComponentAddExpectation {
            channel,
            component,
            exit_code: 1,
            stderr: Some("component failed"),
        },
    );

    let err = toolchain
        .ensure_installed_with(&runner)
        .expect_err("expected component install failure");

    assert!(
        matches!(
            err,
            InstallerError::ToolchainComponentInstallFailed { ref toolchain, ref message, .. }
                if toolchain == "nightly-2025-09-18" && message.contains("component failed")
        ),
        "unexpected error: {err}"
    );
}
