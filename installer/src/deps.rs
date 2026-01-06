//! Dependency installation for Dylint tools.
//!
//! This module provides functions for checking and installing the required
//! `cargo-dylint` and `dylint-link` tools.

use crate::error::{InstallerError, Result};
use std::process::Command;

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
/// use whitaker_installer::deps::check_dylint_tools;
///
/// let status = check_dylint_tools();
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
pub fn check_dylint_tools() -> DylintToolStatus {
    let cargo_dylint = is_cargo_dylint_installed();
    let dylint_link = is_dylint_link_installed();
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
/// * `status` - Current tool status from [`check_dylint_tools`].
///
/// # Errors
///
/// Returns `InstallerError::DependencyInstall` if installation fails.
pub fn install_dylint_tools(status: &DylintToolStatus) -> Result<()> {
    let use_binstall = is_binstall_available();

    if !status.cargo_dylint {
        install_tool("cargo-dylint", use_binstall)?;
    }

    if !status.dylint_link {
        install_tool("dylint-link", use_binstall)?;
    }

    Ok(())
}

/// Checks if `cargo dylint` is available.
fn is_cargo_dylint_installed() -> bool {
    Command::new("cargo")
        .args(["dylint", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Checks if `dylint-link` is in PATH.
fn is_dylint_link_installed() -> bool {
    which::which("dylint-link").is_ok()
}

/// Checks if `cargo binstall` is available.
fn is_binstall_available() -> bool {
    Command::new("cargo")
        .args(["binstall", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Installs a single tool using binstall or cargo install.
fn install_tool(name: &'static str, use_binstall: bool) -> Result<()> {
    let output = if use_binstall {
        Command::new("cargo")
            .args(["binstall", "-y", name])
            .output()?
    } else {
        Command::new("cargo").args(["install", name]).output()?
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn dependency_install_error_includes_tool_name() {
        let err = InstallerError::DependencyInstall {
            tool: "cargo-dylint",
            message: "network error".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("cargo-dylint"));
        assert!(msg.contains("network error"));
    }
}
