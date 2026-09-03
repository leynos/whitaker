//! Test helpers for toolchain tests.

use std::{cell::RefCell, process::ExitStatus};

use super::*;

/// Builds a successful or failed Unix process status for a mock command.
#[cfg(unix)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatusExt::from_raw(code << 8)
}

/// Builds a successful or failed Windows process status for a mock command.
#[cfg(windows)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatusExt::from_raw(code as u32)
}

/// Creates mock command output with the supplied exit status and no output.
pub fn output_with_status(code: i32) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

/// Creates mock command output with the supplied exit status and stderr text.
pub fn output_with_stderr(code: i32, stderr: &str) -> Output {
    Output {
        status: exit_status(code),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// Creates the toolchain value used by installer unit tests.
pub fn test_toolchain(channel: &str) -> Toolchain {
    Toolchain {
        channel: channel.to_owned(),
        workspace_root: Utf8PathBuf::from("."),
    }
}

/// Captures command invocations and returns a preconfigured `Output`.
pub(crate) struct CapturingCommandRunner {
    calls: RefCell<Vec<(String, Vec<String>)>>,
    output: Output,
}

impl CapturingCommandRunner {
    /// Creates a capture runner that clones `output` for every executed command.
    pub(crate) fn new(output: Output) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            output,
        }
    }

    /// Returns a cloned list of recorded `(program, args)` pairs without mutating state.
    pub(crate) fn recorded_calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for CapturingCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<Output> {
        self.calls.borrow_mut().push((
            program.to_owned(),
            args.iter().map(|arg| (*arg).to_owned()).collect(),
        ));
        Ok(self.output.clone())
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
    expectation: &RustupExpectation<'_>,
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
            let output = stderr.as_deref().map_or_else(
                || output_with_status(exit_code),
                |message| output_with_stderr(exit_code, message),
            );
            Ok(output)
        });
}

/// Expected arguments and output for one ordered toolchain installation call.
pub struct ToolchainInstallExpectation<'a> {
    pub channel: &'a str,
    pub exit_code: i32,
    pub stderr: Option<&'a str>,
}

/// Registers an ordered expectation for `rustup run ... rustc --version`.
pub fn expect_rustc_version(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    exit_code: i32,
) {
    let expected_channel = channel.to_owned();
    runner
        .expect_run()
        .withf(move |program, args| {
            let [run, actual_channel, rustc, version] = args else {
                return false;
            };
            program == "rustup"
                && *run == "run"
                && *actual_channel == expected_channel
                && *rustc == "rustc"
                && *version == "--version"
        })
        .times(1)
        .in_sequence(seq)
        .returning(move |_, _| Ok(output_with_status(exit_code)));
}

/// Registers one ordered `rustup toolchain install` expectation.
///
/// The expectation must provide the exact toolchain `channel`, the expected
/// process `exit_code`, and optional standard-error output. The expectation is
/// added to `seq`, so the install command must occur at its declared position
/// in the runner's ordered command sequence.
pub fn expect_toolchain_install(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    expectation: &ToolchainInstallExpectation<'_>,
) {
    let expected_channel = expectation.channel.to_owned();
    expect_rustup_command(
        runner,
        seq,
        &RustupExpectation {
            exit_code: expectation.exit_code,
            stderr: expectation.stderr,
        },
        move |program, args| {
            let [toolchain, install, actual_channel] = args else {
                return false;
            };
            program == "rustup"
                && *toolchain == "toolchain"
                && *install == "install"
                && *actual_channel == expected_channel
        },
    );
}

/// Verifies that an installer operation fails with the expected error.
///
/// The caller supplies mock setup, the operation to invoke, and an error
/// matcher. The helper panics if the operation succeeds or the matcher rejects
/// the returned installer error.
pub fn assert_install_fails_with<F, I, E>(
    toolchain: &Toolchain,
    setup_mocks: F,
    install: I,
    error_matcher: E,
) where
    F: FnOnce(&mut MockCommandRunner, &mut mockall::Sequence),
    I: FnOnce(&Toolchain, &MockCommandRunner) -> Result<ToolchainInstallStatus>,
    E: FnOnce(InstallerError),
{
    let mut runner = MockCommandRunner::new();
    let mut seq = mockall::Sequence::new();

    setup_mocks(&mut runner, &mut seq);

    let err = install(toolchain, &runner).expect_err("expected installation failure");

    error_matcher(err);
}

/// Returns a predicate that matches a rustup component add command with multiple components.
pub fn matches_multi_component_add(
    channel: &str,
    components: &[&str],
) -> impl Fn(&str, &[&str]) -> bool + use<> {
    let expected_channel = channel.to_owned();
    let expected_components: Vec<String> = components.iter().map(|s| (*s).to_owned()).collect();
    move |program, args| {
        let Some((&[component, add, toolchain_flag, actual_channel], actual_components)) =
            args.split_at_checked(4)
        else {
            return false;
        };
        program == "rustup"
            && component == "component"
            && add == "add"
            && toolchain_flag == "--toolchain"
            && actual_channel == expected_channel
            && actual_components.len() == expected_components.len()
            && actual_components
                .iter()
                .zip(&expected_components)
                .all(|(a, b)| *a == b)
    }
}
