//! Tests for Dylint tool dependency checks.

use super::*;
use mockall::Sequence;
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
struct CloneableResult(Result<Output>);

impl CloneableResult {
    fn new(result: Result<Output>) -> Self {
        Self(result)
    }

    fn into_inner(self) -> Result<Output> {
        self.0
    }
}

impl Clone for CloneableResult {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            Ok(output) => Ok(output.clone()),
            Err(err) => Err(clone_installer_error(err)),
        })
    }
}

fn clone_installer_error(err: &InstallerError) -> InstallerError {
    match err {
        InstallerError::ToolchainDetection { reason } => InstallerError::ToolchainDetection {
            reason: reason.clone(),
        },
        InstallerError::ToolchainFileNotFound { path } => {
            InstallerError::ToolchainFileNotFound { path: path.clone() }
        }
        InstallerError::InvalidToolchainFile { reason } => InstallerError::InvalidToolchainFile {
            reason: reason.clone(),
        },
        InstallerError::ToolchainNotInstalled { toolchain } => {
            InstallerError::ToolchainNotInstalled {
                toolchain: toolchain.clone(),
            }
        }
        InstallerError::BuildFailed { crate_name, reason } => InstallerError::BuildFailed {
            crate_name: crate_name.clone(),
            reason: reason.clone(),
        },
        InstallerError::StagingFailed { reason } => InstallerError::StagingFailed {
            reason: reason.clone(),
        },
        InstallerError::TargetNotWritable { path, reason } => InstallerError::TargetNotWritable {
            path: path.clone(),
            reason: reason.clone(),
        },
        InstallerError::LintCrateNotFound { name } => InstallerError::LintCrateNotFound {
            name: name.clone(),
        },
        InstallerError::WorkspaceNotFound { reason } => InstallerError::WorkspaceNotFound {
            reason: reason.clone(),
        },
        InstallerError::InvalidCargoToml { path, reason } => InstallerError::InvalidCargoToml {
            path: path.clone(),
            reason: reason.clone(),
        },
        InstallerError::Io(source) => {
            InstallerError::Io(std::io::Error::new(source.kind(), source.to_string()))
        }
        InstallerError::Git { operation, message } => InstallerError::Git {
            operation: *operation,
            message: message.clone(),
        },
        InstallerError::DependencyInstall { tool, message } => InstallerError::DependencyInstall {
            tool: *tool,
            message: message.clone(),
        },
        InstallerError::WrapperGeneration(message) => {
            InstallerError::WrapperGeneration(message.clone())
        }
        InstallerError::ScanFailed { source } => InstallerError::ScanFailed {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        InstallerError::WriteFailed { source } => InstallerError::WriteFailed {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
    }
}

fn test_check_dylint_tools_with_outcomes(
    cargo_dylint_result: Result<Output>,
    dylint_link_result: Result<Output>,
    expected_status: DylintToolStatus,
) {
    let mut executor = MockCommandExecutor::new();
    let mut sequence = Sequence::new();

    let cargo_dylint_result = CloneableResult::new(cargo_dylint_result);
    let dylint_link_result = CloneableResult::new(dylint_link_result);

    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["dylint", "--version"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(move |_, _| cargo_dylint_result.clone().into_inner());
    executor
        .expect_run()
        .withf(|cmd, args| cmd == "dylint-link" && args == ["--version"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(move |_, _| dylint_link_result.clone().into_inner());

    let status = check_dylint_tools(&executor);

    assert_eq!(status, expected_status);
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
fn install_dylint_tools_uses_binstall_when_available() {
    let mut executor = MockCommandExecutor::new();
    let mut sequence = Sequence::new();

    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "--version"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(success_output()));
    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "-y", "cargo-dylint"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(success_output()));
    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "-y", "dylint-link"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(success_output()));

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install() {
    let mut executor = MockCommandExecutor::new();
    let mut sequence = Sequence::new();

    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "--version"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(failure_output("no binstall")));
    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["install", "cargo-dylint"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(success_output()));

    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };

    let result = install_dylint_tools(&executor, &status);

    assert!(result.is_ok());
}

#[test]
fn install_dylint_tools_reports_install_failure() {
    let mut executor = MockCommandExecutor::new();
    let mut sequence = Sequence::new();

    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "--version"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(success_output()));
    executor
        .expect_run()
        .withf(|cmd, args| cmd == "cargo" && args == ["binstall", "-y", "cargo-dylint"])
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok(failure_output("network down")));

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
}
