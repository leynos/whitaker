//! Toolchain detection and validation for the installer.
//!
//! This module provides utilities to detect the pinned Rust toolchain from
//! `rust-toolchain.toml` and verify that it is installed via rustup.

use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;

/// Represents a detected Rust toolchain configuration.
#[derive(Debug, Clone)]
pub struct Toolchain {
    channel: String,
    workspace_root: Utf8PathBuf,
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
        let channel = parse_toolchain_channel(&contents)?;

        Ok(Self {
            channel,
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
            workspace_root: workspace_root.to_owned(),
        }
    }

    /// Verify that the toolchain is installed via rustup.
    ///
    /// # Errors
    ///
    /// Returns an error if the toolchain is not installed.
    pub fn verify_installed(&self) -> Result<()> {
        let output = Command::new("rustup")
            .args(["run", &self.channel, "rustc", "--version"])
            .output()?;

        if output.status.success() {
            return Ok(());
        }

        Err(InstallerError::ToolchainNotInstalled {
            toolchain: self.channel.clone(),
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
    let table: toml::Table =
        contents
            .parse()
            .map_err(|e| InstallerError::InvalidToolchainFile {
                reason: format!("TOML parse error: {e}"),
            })?;

    // Try [toolchain].channel first (standard format)
    let channel_from_toolchain = table
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(|c| c.as_str());

    if let Some(s) = channel_from_toolchain {
        return Ok(s.to_owned());
    }

    // Fall back to top-level channel key
    let channel_from_top = table.get("channel").and_then(|c| c.as_str());

    if let Some(s) = channel_from_top {
        return Ok(s.to_owned());
    }

    Err(InstallerError::InvalidToolchainFile {
        reason: "no channel field found in rust-toolchain.toml".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_toolchain_format() {
        let contents = r#"
[toolchain]
channel = "nightly-2025-09-18"
components = ["rust-src", "rustc-dev"]
"#;
        let channel = parse_toolchain_channel(contents);
        assert!(channel.is_ok());
        assert_eq!(channel.ok(), Some("nightly-2025-09-18".to_owned()));
    }

    #[test]
    fn parses_simple_channel_format() {
        let contents = r#"channel = "stable""#;
        let channel = parse_toolchain_channel(contents);
        assert!(channel.is_ok());
        assert_eq!(channel.ok(), Some("stable".to_owned()));
    }

    #[test]
    fn rejects_missing_channel() {
        let contents = r#"
[toolchain]
components = ["rust-src"]
"#;
        let result = parse_toolchain_channel(contents);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_toml() {
        let contents = "this is not valid toml {{{";
        let result = parse_toolchain_channel(contents);
        assert!(result.is_err());
    }
}
