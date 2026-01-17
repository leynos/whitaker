//! Dependency installation for Dylint tools.
//!
//! This module provides functions for checking and installing the required
//! `cargo-dylint` and `dylint-link` tools.

use crate::error::{InstallerError, Result};
use std::process::{Command, Output};

/// Abstraction for running external commands.
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
    /// let executor = SystemCommandExecutor;
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
/// let executor = SystemCommandExecutor;
/// let output = executor.run("cargo", &["--version"])?;
/// assert!(output.status.success());
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        Command::new(cmd)
            .args(args)
            .output()
            .map_err(InstallerError::from)
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
/// let executor = SystemCommandExecutor;
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
/// # Usage
///
/// See the module-level documentation for usage patterns and side-effect context.
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

/// Returns true if the given command executes successfully.
fn command_succeeds(executor: &dyn CommandExecutor, cmd: &str, args: &[&str]) -> bool {
    executor.run(cmd, args).is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests;
