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
    let toolchain = Toolchain {
        channel: "nightly-2025-09-18".to_owned(),
        components: Vec::new(),
        workspace_root: Utf8PathBuf::from("."),
    };

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == "nightly-2025-09-18"
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(1)));

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 3
                && args[0] == "toolchain"
                && args[1] == "install"
                && args[2] == "nightly-2025-09-18"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == "nightly-2025-09-18"
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    let status = toolchain
        .ensure_installed_with(&runner)
        .expect("toolchain should install");

    assert!(status.installed_toolchain());
}

#[test]
fn ensure_installed_adds_components_when_present() {
    let toolchain = Toolchain {
        channel: "nightly-2025-09-18".to_owned(),
        components: vec!["rust-src".to_owned()],
        workspace_root: Utf8PathBuf::from("."),
    };

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == "nightly-2025-09-18"
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 5
                && args[0] == "component"
                && args[1] == "add"
                && args[2] == "--toolchain"
                && args[3] == "nightly-2025-09-18"
                && args[4] == "rust-src"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    let status = toolchain
        .ensure_installed_with(&runner)
        .expect("toolchain should be ready");

    assert!(!status.installed_toolchain());
}

#[test]
fn ensure_installed_reports_toolchain_install_failure() {
    let toolchain = Toolchain {
        channel: "nightly-2025-09-18".to_owned(),
        components: Vec::new(),
        workspace_root: Utf8PathBuf::from("."),
    };

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == "nightly-2025-09-18"
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(1)));

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 3
                && args[0] == "toolchain"
                && args[1] == "install"
                && args[2] == "nightly-2025-09-18"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_stderr(1, "network down")));

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
    let toolchain = Toolchain {
        channel: "nightly-2025-09-18".to_owned(),
        components: vec!["rust-src".to_owned()],
        workspace_root: Utf8PathBuf::from("."),
    };

    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 4
                && args[0] == "run"
                && args[1] == "nightly-2025-09-18"
                && args[2] == "rustc"
                && args[3] == "--version"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_status(0)));

    runner
        .expect_run()
        .withf(|program, args| {
            program == "rustup"
                && args.len() == 5
                && args[0] == "component"
                && args[1] == "add"
                && args[2] == "--toolchain"
                && args[3] == "nightly-2025-09-18"
                && args[4] == "rust-src"
        })
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(output_with_stderr(1, "component failed")));

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
