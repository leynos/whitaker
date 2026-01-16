//! Tests for Dylint tool dependency checks.

use super::*;
use crate::test_utils::*;

/// Helper to test `check_dylint_tools` with various tool detection outcomes.
///
/// Sets up a `StubExecutor` with expected calls for checking cargo-dylint and dylint-link
/// availability, then verifies the returned status matches the expected value.
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

/// Helper to test `command_succeeds` with a given command result and expected outcome.
///
/// Sets up a `StubExecutor` with a single expected call for "cargo dylint --version" using
/// the provided result, then asserts the return value matches the expected boolean.
fn test_command_succeeds_with_result(command_result: Result<Output>, expected: bool) {
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: command_result,
    }]);

    assert_eq!(
        command_succeeds(&executor, "cargo", &["dylint", "--version"]),
        expected
    );
    executor.assert_finished();
}

/// Helper to test successful `install_dylint_tools` invocations.
///
/// Sets up a `StubExecutor` with the provided expected calls and verifies that
/// `install_dylint_tools` returns `Ok(())` with the given tool status.
fn test_install_dylint_tools_success(expected_calls: Vec<ExpectedCall>, status: DylintToolStatus) {
    let executor = StubExecutor::new(expected_calls);
    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
    executor.assert_finished();
}

/// Helper to test failed `install_dylint_tools` invocations.
///
/// Sets up a `StubExecutor` with the provided expected calls and verifies that
/// `install_dylint_tools` returns `Err(InstallerError::DependencyInstall)` with the
/// expected tool name and error message.
fn test_install_dylint_tools_failure(
    expected_calls: Vec<ExpectedCall>,
    status: DylintToolStatus,
    expected_tool: &str,
    expected_message: &str,
) {
    let executor = StubExecutor::new(expected_calls);
    let err = match install_dylint_tools(&executor, &status) {
        Ok(()) => panic!("expected install failure"),
        Err(err) => err,
    };

    match err {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, expected_tool);
            assert_eq!(message, expected_message);
        }
        other => panic!("unexpected error: {other}"),
    }
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
    test_command_succeeds_with_result(Ok(success_output()), true);
}

#[test]
fn command_succeeds_returns_false_on_failure_output() {
    test_command_succeeds_with_result(Ok(failure_output("no dylint")), false);
}

#[test]
fn command_succeeds_returns_false_on_error() {
    test_command_succeeds_with_result(Err(std::io::Error::other("missing dylint").into()), false);
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
    test_install_dylint_tools_success(
        vec![
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
        ],
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        },
    );
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install() {
    test_install_dylint_tools_success(
        vec![
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
        ],
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
    );
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install_on_binstall_error() {
    test_install_dylint_tools_success(
        vec![
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
        ],
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
    );
}

#[test]
fn install_dylint_tools_reports_install_failure() {
    test_install_dylint_tools_failure(
        vec![
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
        ],
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
        "cargo-dylint",
        "network down",
    );
}

#[test]
fn install_dylint_tools_reports_dylint_link_install_failure() {
    test_install_dylint_tools_failure(
        vec![
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
        ],
        DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        },
        "dylint-link",
        "dylint-link failed",
    );
}
