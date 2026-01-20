//! Toolchain detection and validation for the installer.
//!
//! This module provides utilities to detect the pinned Rust toolchain from
//! `rust-toolchain.toml` and verify that it is installed via rustup.

use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::{Command, Output};

/// Represents a detected Rust toolchain configuration.
#[derive(Debug, Clone)]
pub struct Toolchain {
    channel: String,
    components: Vec<String>,
    workspace_root: Utf8PathBuf,
}

/// Status describing whether a toolchain install occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolchainInstallStatus {
    installed_toolchain: bool,
}

impl ToolchainInstallStatus {
    /// Returns true if the toolchain was installed during this run.
    #[must_use]
    pub fn installed_toolchain(&self) -> bool {
        self.installed_toolchain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolchainConfig {
    channel: String,
    components: Vec<String>,
}

/// Abstraction for running external commands.
#[cfg_attr(test, mockall::automock)]
trait CommandRunner {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output>;
}

struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        Command::new(program).args(args).output()
    }
}

impl Toolchain {
    /// Detect the pinned toolchain from `rust-toolchain.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the toolchain file is not found or cannot be parsed.
    pub fn detect(workspace_root: &Utf8Path) -> Result<Self> {
        let toolchain_path = workspace_root.join("rust-toolchain.toml");

        if !toolchain_path.exists() {
            return Err(InstallerError::ToolchainFileNotFound {
                path: toolchain_path,
            });
        }

        let contents = std::fs::read_to_string(&toolchain_path)?;
        let config = parse_toolchain_config(&contents)?;

        Ok(Self {
            channel: config.channel,
            components: config.components,
            workspace_root: workspace_root.to_owned(),
        })
    }

    /// Create a toolchain with an explicit override channel.
    ///
    /// This constructor does not perform validation; callers are responsible
    /// for ensuring the `workspace_root` and `channel` are valid.
    #[must_use]
    pub fn with_override(workspace_root: &Utf8Path, channel: &str) -> Self {
        Self {
            channel: channel.to_owned(),
            components: Vec::new(),
            workspace_root: workspace_root.to_owned(),
        }
    }

    /// Verify that the toolchain is installed via rustup.
    ///
    /// # Errors
    ///
    /// Returns an error if rustup is not found or the toolchain is not installed.
    pub fn verify_installed(&self) -> Result<()> {
        let runner = SystemCommandRunner;
        if self.is_installed_with(&runner)? {
            Ok(())
        } else {
            Err(InstallerError::ToolchainNotInstalled {
                toolchain: self.channel.clone(),
            })
        }
    }

    /// Install the toolchain via rustup if it is missing.
    ///
    /// # Errors
    ///
    /// Returns an error if rustup fails to install the toolchain or required
    /// components.
    pub fn ensure_installed(&self) -> Result<ToolchainInstallStatus> {
        let runner = SystemCommandRunner;
        self.ensure_installed_with(&runner)
    }

    fn ensure_installed_with(&self, runner: &dyn CommandRunner) -> Result<ToolchainInstallStatus> {
        let is_installed = self.is_installed_with(runner)?;
        let mut installed_toolchain = false;

        if !is_installed {
            self.install_toolchain_with(runner)?;
            installed_toolchain = true;
        }

        self.install_components_with(runner)?;

        if !is_installed && !self.is_installed_with(runner)? {
            return Err(InstallerError::ToolchainNotInstalled {
                toolchain: self.channel.clone(),
            });
        }

        Ok(ToolchainInstallStatus {
            installed_toolchain,
        })
    }

    /// Return the channel string for `cargo +<toolchain>` invocations.
    #[must_use]
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Return the workspace root path.
    #[must_use]
    pub fn workspace_root(&self) -> &Utf8Path {
        &self.workspace_root
    }

    fn is_installed_with(&self, runner: &dyn CommandRunner) -> Result<bool> {
        let args = vec![
            "run".to_owned(),
            self.channel.clone(),
            "rustc".to_owned(),
            "--version".to_owned(),
        ];
        let output = run_rustup(runner, &args)?;
        Ok(output.status.success())
    }

    fn install_toolchain_with(&self, runner: &dyn CommandRunner) -> Result<()> {
        let args = vec![
            "toolchain".to_owned(),
            "install".to_owned(),
            self.channel.clone(),
        ];
        let output = run_rustup(runner, &args)?;

        if output.status.success() {
            return Ok(());
        }

        Err(InstallerError::ToolchainInstallFailed {
            toolchain: self.channel.clone(),
            message: stderr_message(&output),
        })
    }

    fn install_components_with(&self, runner: &dyn CommandRunner) -> Result<()> {
        if self.components.is_empty() {
            return Ok(());
        }

        let mut args = vec![
            "component".to_owned(),
            "add".to_owned(),
            "--toolchain".to_owned(),
            self.channel.clone(),
        ];
        args.extend(self.components.iter().cloned());

        let output = run_rustup(runner, &args)?;

        if output.status.success() {
            return Ok(());
        }

        Err(InstallerError::ToolchainComponentInstallFailed {
            toolchain: self.channel.clone(),
            components: self.components.join(", "),
            message: stderr_message(&output),
        })
    }
}

/// Parse the channel from `rust-toolchain.toml` contents.
///
/// This function supports two formats:
/// 1. Standard format with `[toolchain].channel`
/// 2. Simple format with a top-level `channel` key
///
/// # Errors
///
/// Returns an error if the TOML is invalid or no channel field is found.
pub fn parse_toolchain_channel(contents: &str) -> Result<String> {
    parse_toolchain_config(contents).map(|config| config.channel)
}

fn parse_toolchain_config(contents: &str) -> Result<ToolchainConfig> {
    let table: toml::Table =
        contents
            .parse()
            .map_err(|e| InstallerError::InvalidToolchainFile {
                reason: format!("TOML parse error: {e}"),
            })?;

    let channel = parse_channel_from_table(&table)?;
    let components = parse_components_from_table(&table)?;

    Ok(ToolchainConfig {
        channel,
        components,
    })
}

fn parse_channel_from_table(table: &toml::Table) -> Result<String> {
    let channel_from_toolchain = table
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(|c| c.as_str());

    if let Some(s) = channel_from_toolchain {
        return Ok(s.to_owned());
    }

    let channel_from_top = table.get("channel").and_then(|c| c.as_str());

    if let Some(s) = channel_from_top {
        return Ok(s.to_owned());
    }

    Err(InstallerError::InvalidToolchainFile {
        reason: "no channel field found in rust-toolchain.toml".to_owned(),
    })
}

fn parse_components_from_table(table: &toml::Table) -> Result<Vec<String>> {
    if let Some(value) = table
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("components"))
    {
        return parse_components_value(value);
    }

    if let Some(value) = table.get("components") {
        return parse_components_value(value);
    }

    Ok(Vec::new())
}

fn parse_components_value(value: &toml::Value) -> Result<Vec<String>> {
    let components = value
        .as_array()
        .ok_or_else(|| InstallerError::InvalidToolchainFile {
            reason: "components must be an array of strings".to_owned(),
        })?;

    components
        .iter()
        .map(|component| {
            component.as_str().map(str::to_owned).ok_or_else(|| {
                InstallerError::InvalidToolchainFile {
                    reason: "components must be an array of strings".to_owned(),
                }
            })
        })
        .collect()
}

fn run_rustup(runner: &dyn CommandRunner, args: &[String]) -> Result<Output> {
    runner
        .run("rustup", args)
        .map_err(|e| InstallerError::ToolchainDetection {
            reason: format!("failed to run rustup: {e}"),
        })
}

fn stderr_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        "unknown error".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests;
