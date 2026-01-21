//! Test helpers for toolchain tests.

use super::*;
use std::process::ExitStatus;

#[cfg(unix)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatusExt::from_raw(code << 8)
}

#[cfg(windows)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatusExt::from_raw(code as u32)
}

pub fn output_with_status(code: i32) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

pub fn output_with_stderr(code: i32, stderr: &str) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

pub fn test_toolchain(channel: &str) -> Toolchain {
    Toolchain {
        channel: channel.to_owned(),
        workspace_root: Utf8PathBuf::from("."),
    }
}

/// Common expectation fields for rustup commands.
pub struct RustupExpectation<'a> {
    pub exit_code: i32,
    pub stderr: Option<&'a str>,
}

// Generic helper to expect a rustup command with custom validation.
fn expect_rustup_command<F>(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    expectation: RustupExpectation<'_>,
    matcher: F,
) where
    F: Fn(&str, &[&str]) -> bool + Send + 'static,
{
    let stderr = expectation.stderr.map(str::to_owned);
    let exit_code = expectation.exit_code;
    runner
        .expect_run()
        .withf(matcher)
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

pub struct ToolchainInstallExpectation<'a> {
    pub channel: &'a str,
    pub exit_code: i32,
    pub stderr: Option<&'a str>,
}

pub fn expect_rustc_version(
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

pub fn expect_toolchain_install(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    expectation: ToolchainInstallExpectation<'_>,
) {
    let channel = expectation.channel.to_owned();
    expect_rustup_command(
        runner,
        seq,
        RustupExpectation {
            exit_code: expectation.exit_code,
            stderr: expectation.stderr,
        },
        move |program, args| {
            program == "rustup"
                && args.len() == 3
                && args[0] == "toolchain"
                && args[1] == "install"
                && args[2] == channel
        },
    );
}

// Helper to test that ensure_installed fails with the expected error.
pub fn assert_install_fails_with<F, E>(toolchain: Toolchain, setup_mocks: F, error_matcher: E)
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

// Returns a predicate that matches a rustup component add command with multiple components.
pub fn matches_multi_component_add(
    channel: &str,
    components: &[&str],
) -> impl Fn(&str, &[&str]) -> bool {
    let channel = channel.to_owned();
    let components: Vec<String> = components.iter().map(|s| (*s).to_owned()).collect();
    move |program, args| {
        program == "rustup"
            && args.len() == 4 + components.len()
            && args[0] == "component"
            && args[1] == "add"
            && args[2] == "--toolchain"
            && args[3] == channel
            && args[4..].iter().zip(&components).all(|(a, b)| *a == b)
    }
}
