//! Error types for the Whitaker installer CLI.
//!
//! This module defines semantic error variants that provide actionable guidance
//! to users when installation fails. Each error includes recovery hints where
//! applicable.

use crate::builder::CrateName;
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
        crate_name: CrateName,
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
        name: CrateName,
    },

    /// The workspace root could not be found.
    #[error("workspace not found: {reason}")]
    WorkspaceNotFound {
        /// Description of why the workspace was not found.
        reason: String,
    },

    /// A Cargo.toml file could not be parsed during workspace detection.
    #[error("invalid Cargo.toml at {path}: {reason}")]
    InvalidCargoToml {
        /// Path to the invalid Cargo.toml.
        path: camino::Utf8PathBuf,
        /// Description of the parse error.
        reason: String,
    },

    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Git clone or update operation failed.
    #[error("git {operation} failed: {message}")]
    Git {
        /// The git operation that failed (clone, pull, etc.).
        operation: &'static str,
        /// Description of the failure.
        message: String,
    },

    /// Required tool installation failed.
    #[error("failed to install {tool}: {message}")]
    DependencyInstall {
        /// Name of the tool that failed to install.
        tool: &'static str,
        /// Description of the installation failure.
        message: String,
    },

    /// Wrapper script generation failed.
    #[error("wrapper script generation failed: {0}")]
    WrapperGeneration(String),

    /// Failed to scan the staging directory for installed lints.
    #[error("failed to scan staging directory: {reason}")]
    ScanFailed {
        /// Description of the scan failure.
        reason: String,
    },
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
            crate_name: CrateName::from("module_max_lines"),
            reason: "compilation error".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("module_max_lines"));
        assert!(msg.contains("compilation error"));
    }

    #[test]
    fn git_error_includes_operation_and_message() {
        let err = InstallerError::Git {
            operation: "clone",
            message: "network error".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("clone"));
        assert!(msg.contains("network error"));
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

    #[test]
    fn wrapper_generation_error_includes_message() {
        let err = InstallerError::WrapperGeneration("permission denied".to_owned());
        let msg = err.to_string();
        assert!(msg.contains("permission denied"));
    }
}
