//! Tests for the installer CLI entrypoint.

use super::*;
use rstest::rstest;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::process::{ExitStatus, Output};
use whitaker_installer::cli::InstallArgs;

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code as u32)
}

fn success_output() -> Output {
    Output {
        status: exit_status(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn failure_output(stderr: &str) -> Output {
    Output {
        status: exit_status(1),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[derive(Debug)]
struct ExpectedCall {
    cmd: &'static str,
    args: Vec<&'static str>,
    result: Result<Output>,
}

#[derive(Debug)]
struct StubExecutor {
    expected: RefCell<VecDeque<ExpectedCall>>,
}

impl StubExecutor {
    fn new(expected: Vec<ExpectedCall>) -> Self {
        Self {
            expected: RefCell::new(expected.into()),
        }
    }

    fn assert_finished(&self) {
        assert!(
            self.expected.borrow().is_empty(),
            "expected no further command invocations"
        );
    }
}

impl CommandExecutor for StubExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let mut expected = self.expected.borrow_mut();
        let call = expected.pop_front().expect("unexpected command invocation");

        assert_eq!(call.cmd, cmd);
        assert_eq!(call.args.as_slice(), args);

        call.result
    }
}

#[test]
fn exit_code_for_run_result_returns_zero_on_success() {
    let mut stderr = Vec::new();
    let exit_code = exit_code_for_run_result(Ok(()), &mut stderr);
    assert_eq!(exit_code, 0);
    assert!(stderr.is_empty());
}

#[test]
fn exit_code_for_run_result_prints_error_and_returns_one() {
    let err = InstallerError::LintCrateNotFound {
        name: CrateName::from("nonexistent_lint"),
    };

    let mut stderr = Vec::new();
    let exit_code = exit_code_for_run_result(Err(err), &mut stderr);
    assert_eq!(exit_code, 1);

    let stderr_text = String::from_utf8(stderr).expect("stderr was not UTF-8");
    assert!(stderr_text.contains("lint crate nonexistent_lint not found"));
}

#[rstest]
#[case::default_suite_only(InstallArgs::default(), false, true)]
#[case::individual_lints(
    InstallArgs { individual_lints: true, ..InstallArgs::default() },
    true,
    false
)]
fn resolve_requested_crates_respects_individual_lints_flag(
    #[case] args: InstallArgs,
    #[case] expect_lint: bool,
    #[case] expect_suite: bool,
) {
    let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
    assert_eq!(
        crates.contains(&CrateName::from("module_max_lines")),
        expect_lint
    );
    assert_eq!(
        crates.contains(&CrateName::from("whitaker_suite")),
        expect_suite
    );
}

#[test]
fn resolve_requested_crates_returns_specific_lints_when_provided() {
    let args = InstallArgs {
        lint: vec!["module_max_lines".to_owned()],
        ..InstallArgs::default()
    };

    let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
    assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
}

#[test]
fn resolve_requested_crates_rejects_unknown_lints() {
    let args = InstallArgs {
        lint: vec!["nonexistent_lint".to_owned()],
        ..InstallArgs::default()
    };

    let err = resolve_requested_crates(&args).expect_err("expected crate resolution to fail");
    assert!(matches!(
        err,
        InstallerError::LintCrateNotFound { name } if name == CrateName::from("nonexistent_lint")
    ));
}

#[test]
fn ensure_dylint_tools_skips_install_when_installed() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(success_output()),
        },
    ]);

    let mut stderr = Vec::new();
    let result = ensure_dylint_tools_with_executor(&executor, false, &mut stderr);

    assert!(result.is_ok());
    assert!(stderr.is_empty());
    executor.assert_finished();
}

#[test]
fn ensure_dylint_tools_installs_missing_tools_and_logs() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(failure_output("missing cargo-dylint")),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(failure_output("missing dylint-link")),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "dylint-link"],
            result: Ok(success_output()),
        },
    ]);

    let mut stderr = Vec::new();
    let result = ensure_dylint_tools_with_executor(&executor, false, &mut stderr);

    assert!(result.is_ok());

    let stderr_text = String::from_utf8(stderr).expect("stderr was not UTF-8");
    assert!(stderr_text.contains("Installing required Dylint tools..."));
    assert!(stderr_text.contains("Dylint tools installed successfully."));
    assert!(stderr_text.ends_with("\n\n"));

    executor.assert_finished();
}

#[test]
fn ensure_dylint_tools_quiet_suppresses_output() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(failure_output("missing cargo-dylint")),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(failure_output("missing dylint-link")),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "dylint-link"],
            result: Ok(success_output()),
        },
    ]);

    let mut stderr = Vec::new();
    let result = ensure_dylint_tools_with_executor(&executor, true, &mut stderr);

    assert!(result.is_ok());
    assert!(stderr.is_empty());
    executor.assert_finished();
}

#[test]
fn ensure_dylint_tools_propagates_install_failures() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(failure_output("missing cargo-dylint")),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(failure_output("install failed")),
        },
    ]);

    let mut stderr = Vec::new();
    let err = ensure_dylint_tools_with_executor(&executor, false, &mut stderr)
        .expect_err("expected install failure");

    assert!(matches!(
        err,
        InstallerError::DependencyInstall { tool, message }
            if tool == "cargo-dylint" && message == "install failed"
    ));
    executor.assert_finished();
}
