//! Git operations for cloning and updating the Whitaker repository.
//!
//! This module provides functions for managing the local Whitaker clone,
//! including initial cloning and subsequent updates.

use crate::error::{InstallerError, Result};
use crate::workspace::WHITAKER_REPO_URL;
use camino::Utf8Path;
use std::process::Command;

/// Clones the Whitaker repository to the specified target directory.
///
/// Creates the parent directories if they do not exist.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the clone fails.
pub fn clone_repository(target: &Utf8Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new("git")
        .args(["clone", WHITAKER_REPO_URL, target.as_str()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "clone",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
}

/// Updates an existing Whitaker repository by pulling the latest changes.
///
/// Runs `git pull` in the specified repository directory.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the pull fails.
pub fn update_repository(repo: &Utf8Path) -> Result<()> {
    let output = Command::new("git")
        .args(["pull"])
        .current_dir(repo.as_std_path())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "pull",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_repository_error_includes_operation() {
        let err = InstallerError::Git {
            operation: "clone",
            message: "test error".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("clone"));
        assert!(msg.contains("test error"));
    }

    #[test]
    fn update_repository_error_includes_operation() {
        let err = InstallerError::Git {
            operation: "pull",
            message: "not a git repository".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("pull"));
        assert!(msg.contains("not a git repository"));
    }
}
