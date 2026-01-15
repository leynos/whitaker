//! Tests for Dylint tool dependency checks.

use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::process::ExitStatus;

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

fn test_check_dylint_tools_with_outcomes(
    cargo_dylint_result: Result<Output>,
    dylint_link_result: Result<Output>,
    expected_status: DylintToolStatus,
) {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: cargo_dylint_result,
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: dylint_link_result,
        },
    ]);

    let status = check_dylint_tools(&executor);

    assert_eq!(status, expected_status);
    executor.assert_finished();
}

#[test]
fn dylint_tool_status_all_installed_when_both_present() {
    let status = DylintToolStatus {
        cargo_dylint: true,
        dylint_link: true,
    };
    assert!(status.all_installed());
}

#[test]
fn dylint_tool_status_not_all_installed_when_one_missing() {
    let status = DylintToolStatus {
        cargo_dylint: true,
        dylint_link: false,
    };
    assert!(!status.all_installed());

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };
    assert!(!status.all_installed());
}

#[test]
fn command_succeeds_returns_true_on_success() {
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(success_output()),
    }]);

    assert!(command_succeeds(
        &executor,
        "cargo",
        &["dylint", "--version"]
    ));
    executor.assert_finished();
}

#[test]
fn command_succeeds_returns_false_on_failure_output() {
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(failure_output("no dylint")),
    }]);

    assert!(!command_succeeds(
        &executor,
        "cargo",
        &["dylint", "--version"]
    ));
    executor.assert_finished();
}

#[test]
fn command_succeeds_returns_false_on_error() {
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Err(std::io::Error::other("missing dylint").into()),
    }]);

    assert!(!command_succeeds(
        &executor,
        "cargo",
        &["dylint", "--version"]
    ));
    executor.assert_finished();
}

#[test]
fn system_command_executor_runs_command() {
    let executor = SystemCommandExecutor;
    let output = executor
        .run("cargo", &["--version"])
        .expect("expected cargo to be available");

    assert!(output.status.success());
}

#[test]
fn check_dylint_tools_reports_installed_tools() {
    test_check_dylint_tools_with_outcomes(
        Ok(success_output()),
        Ok(success_output()),
        DylintToolStatus {
            cargo_dylint: true,
            dylint_link: true,
        },
    );
}

#[test]
fn check_dylint_tools_reports_missing_tools() {
    test_check_dylint_tools_with_outcomes(
        Ok(failure_output("no dylint")),
        Err(std::io::Error::other("missing dylint-link").into()),
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        },
    );
}

#[test]
fn check_dylint_tools_reports_missing_dylint_link() {
    test_check_dylint_tools_with_outcomes(
        Ok(success_output()),
        Ok(failure_output("missing dylint-link")),
        DylintToolStatus {
            cargo_dylint: true,
            dylint_link: false,
        },
    );
}

#[test]
fn check_dylint_tools_reports_missing_cargo_dylint() {
    test_check_dylint_tools_with_outcomes(
        Ok(failure_output("missing cargo-dylint")),
        Ok(success_output()),
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
    );
}

#[test]
fn install_dylint_tools_uses_binstall_when_available() {
    let executor = StubExecutor::new(vec![
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

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(failure_output("no binstall")),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "cargo-dylint"],
            result: Ok(success_output()),
        },
    ]);

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };

    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install_on_binstall_error() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Err(std::io::Error::other("failed to execute cargo binstall").into()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "cargo-dylint"],
            result: Ok(success_output()),
        },
    ]);

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };

    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_reports_install_failure() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(failure_output("network down")),
        },
    ]);

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };

    let err = match install_dylint_tools(&executor, &status) {
        Ok(()) => panic!("expected install failure"),
        Err(err) => err,
    };

    match err {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, "cargo-dylint");
            assert_eq!(message, "network down");
        }
        other => panic!("unexpected error: {other}"),
    }
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_reports_dylint_link_install_failure() {
    let executor = StubExecutor::new(vec![
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
            result: Ok(failure_output("dylint-link failed")),
        },
    ]);

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    let err = match install_dylint_tools(&executor, &status) {
        Ok(()) => panic!("expected install failure"),
        Err(err) => err,
    };

    match err {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, "dylint-link");
            assert_eq!(message, "dylint-link failed");
        }
        other => panic!("unexpected error: {other}"),
    }
    executor.assert_finished();
}
