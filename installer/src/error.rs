//! Error types for the Whitaker installer CLI.
//!
//! This module defines semantic error variants that provide actionable guidance
//! to users when installation fails. Each error includes recovery hints where
//! applicable.

use camino::Utf8PathBuf;
use thiserror::Error;

/// Errors that can occur during the installation process.
#[derive(Debug, Error)]
pub enum InstallerError {
    /// Failed to detect the Rust toolchain from configuration.
    #[error("toolchain detection failed: {reason}")]
    ToolchainDetection {
        /// Description of why detection failed.
        reason: String,
    },

    /// The `rust-toolchain.toml` file was not found at the expected location.
    #[error("rust-toolchain.toml not found at {path}")]
    ToolchainFileNotFound {
        /// Path where the file was expected.
        path: Utf8PathBuf,
    },

    /// The `rust-toolchain.toml` file could not be parsed.
    #[error("invalid rust-toolchain.toml: {reason}")]
    InvalidToolchainFile {
        /// Description of the parse error.
        reason: String,
    },

    /// The required toolchain is not installed via rustup.
    #[error("toolchain {toolchain} not installed; run: rustup toolchain install {toolchain}")]
    ToolchainNotInstalled {
        /// The missing toolchain channel.
        toolchain: String,
    },

    /// A cargo build command failed for a lint crate.
    #[error("cargo build failed for {crate_name}: {reason}")]
    BuildFailed {
        /// Name of the crate that failed to build.
        crate_name: String,
        /// Description of the build failure.
        reason: String,
    },

    /// Failed to stage libraries to the target directory.
    #[error("staging failed: {reason}")]
    StagingFailed {
        /// Description of the staging failure.
        reason: String,
    },

    /// The target directory exists but is not writable.
    #[error("target directory {path} is not writable: {reason}")]
    TargetNotWritable {
        /// Path to the non-writable directory.
        path: Utf8PathBuf,
        /// Description of the underlying I/O error.
        reason: String,
    },

    /// A specified lint crate was not found in the workspace.
    #[error("lint crate {name} not found in workspace")]
    LintCrateNotFound {
        /// Name of the missing lint crate.
        name: String,
    },

    /// The workspace root could not be found.
    #[error("workspace not found: {reason}")]
    WorkspaceNotFound {
        /// Description of why the workspace was not found.
        reason: String,
    },

    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias using [`InstallerError`].
pub type Result<T> = std::result::Result<T, InstallerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolchain_not_installed_suggests_install_command() {
        let err = InstallerError::ToolchainNotInstalled {
            toolchain: "nightly-2025-09-18".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("rustup toolchain install"));
        assert!(msg.contains("nightly-2025-09-18"));
    }

    #[test]
    fn build_failed_includes_crate_name() {
        let err = InstallerError::BuildFailed {
            crate_name: "module_max_lines".to_owned(),
            reason: "compilation error".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("module_max_lines"));
        assert!(msg.contains("compilation error"));
    }
}
