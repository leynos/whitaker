//! Dependency installation for Dylint tools.
//!
//! This module provides functions for checking and installing the required
//! `cargo-dylint` and `dylint-link` tools.

use crate::error::{InstallerError, Result};
use std::process::{Command, Output};

/// Abstraction for running external commands.
#[cfg_attr(test, mockall::automock)]
pub trait CommandExecutor {
    /// Runs a command with arguments and returns the captured output.
    ///
    /// # Errors
    ///
    /// Returns any I/O errors encountered while spawning or running the command.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use whitaker_installer::deps::{CommandExecutor, SystemCommandExecutor};
    ///
    /// let executor = SystemCommandExecutor::default();
    /// let output = executor.run("cargo", &["--version"])?;
    /// assert!(output.status.success());
    /// # Ok::<(), whitaker_installer::error::InstallerError>(())
    /// ```
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>;
}

/// Executes commands on the host system.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::deps::{CommandExecutor, SystemCommandExecutor};
///
/// let executor = SystemCommandExecutor::default();
/// let output = executor.run("cargo", &["--version"])?;
/// assert!(output.status.success());
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        Command::new(cmd).args(args).output().map_err(InstallerError::from)
    }
}

/// Status of Dylint tool availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DylintToolStatus {
    /// Whether `cargo-dylint` is installed.
    pub cargo_dylint: bool,
    /// Whether `dylint-link` is installed.
    pub dylint_link: bool,
}

impl DylintToolStatus {
    /// Returns `true` if both tools are installed.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::deps::DylintToolStatus;
    ///
    /// let status = DylintToolStatus {
    ///     cargo_dylint: true,
    ///     dylint_link: true,
    /// };
    /// assert!(status.all_installed());
    ///
    /// let partial = DylintToolStatus {
    ///     cargo_dylint: true,
    ///     dylint_link: false,
    /// };
    /// assert!(!partial.all_installed());
    /// ```
    pub fn all_installed(&self) -> bool {
        self.cargo_dylint && self.dylint_link
    }
}

/// Checks whether the Dylint tools are installed.
///
/// Returns a status struct indicating which tools are available.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::deps::{check_dylint_tools, SystemCommandExecutor};
///
/// let executor = SystemCommandExecutor::default();
/// let status = check_dylint_tools(&executor);
/// if status.all_installed() {
///     println!("All Dylint tools are available");
/// } else {
///     if !status.cargo_dylint {
///         println!("cargo-dylint is missing");
///     }
///     if !status.dylint_link {
///         println!("dylint-link is missing");
///     }
/// }
/// ```
pub fn check_dylint_tools(executor: &dyn CommandExecutor) -> DylintToolStatus {
    let cargo_dylint = is_cargo_dylint_installed(executor);
    let dylint_link = is_dylint_link_installed(executor);
    DylintToolStatus {
        cargo_dylint,
        dylint_link,
    }
}

/// Installs missing Dylint tools.
///
/// Uses `cargo binstall` if available for faster installation, otherwise
/// falls back to `cargo install`.
///
/// # Arguments
///
/// * `executor` - Command executor for running install checks and installers.
/// * `status` - Current tool status from [`check_dylint_tools`].
///
/// # Errors
///
/// Returns `InstallerError::DependencyInstall` if installation fails.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::deps::{check_dylint_tools, install_dylint_tools, SystemCommandExecutor};
///
/// let executor = SystemCommandExecutor::default();
/// let status = check_dylint_tools(&executor);
/// install_dylint_tools(&executor, &status)?;
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn install_dylint_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
) -> Result<()> {
    let use_binstall = is_binstall_available(executor);

    if !status.cargo_dylint {
        install_tool(executor, "cargo-dylint", use_binstall)?;
    }

    if !status.dylint_link {
        install_tool(executor, "dylint-link", use_binstall)?;
    }

    Ok(())
}

/// Checks if `cargo dylint` is available.
fn is_cargo_dylint_installed(executor: &dyn CommandExecutor) -> bool {
    command_succeeds(executor, "cargo", &["dylint", "--version"])
}

/// Checks if `dylint-link` is in PATH.
fn is_dylint_link_installed(executor: &dyn CommandExecutor) -> bool {
    command_succeeds(executor, "dylint-link", &["--version"])
}

/// Checks if `cargo binstall` is available.
fn is_binstall_available(executor: &dyn CommandExecutor) -> bool {
    command_succeeds(executor, "cargo", &["binstall", "--version"])
}

/// Installs a single tool using binstall or cargo install.
fn install_tool(
    executor: &dyn CommandExecutor,
    name: &'static str,
    use_binstall: bool,
) -> Result<()> {
    let output = if use_binstall {
        executor.run("cargo", &["binstall", "-y", name])?
    } else {
        executor.run("cargo", &["install", name])?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::DependencyInstall {
            tool: name,
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
}

fn command_succeeds(executor: &dyn CommandExecutor, cmd: &str, args: &[&str]) -> bool {
    executor.run(cmd, args).is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests {
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
        let mut executor = MockCommandExecutor::new();
        let mut sequence = Sequence::new();

        executor
            .expect_run()
            .withf(|cmd, args| cmd == "cargo" && args == ["dylint", "--version"])
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(success_output()));
        executor
            .expect_run()
            .withf(|cmd, args| cmd == "dylint-link" && args == ["--version"])
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(success_output()));

        let status = check_dylint_tools(&executor);

        assert_eq!(
            status,
            DylintToolStatus {
                cargo_dylint: true,
                dylint_link: true,
            }
        );
    }

    #[test]
    fn check_dylint_tools_reports_missing_tools() {
        let mut executor = MockCommandExecutor::new();
        let mut sequence = Sequence::new();

        executor
            .expect_run()
            .withf(|cmd, args| cmd == "cargo" && args == ["dylint", "--version"])
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(failure_output("no dylint")));
        executor
            .expect_run()
            .withf(|cmd, args| cmd == "dylint-link" && args == ["--version"])
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|_, _| Err(std::io::Error::other("missing dylint-link").into()));

        let status = check_dylint_tools(&executor);

        assert_eq!(
            status,
            DylintToolStatus {
                cargo_dylint: false,
                dylint_link: false,
            }
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
}
