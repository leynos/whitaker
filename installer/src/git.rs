//! Git operations for cloning and updating the Whitaker repository.
//!
//! This module provides functions for managing the local Whitaker clone,
//! including initial cloning and subsequent updates. Operations have a
//! configurable timeout to prevent hangs on network issues.

use crate::error::{InstallerError, Result};
use crate::workspace::WHITAKER_REPO_URL;
use camino::Utf8Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Default timeout for git operations (5 minutes).
const GIT_TIMEOUT: Duration = Duration::from_secs(300);

/// Clones the Whitaker repository to the specified target directory.
///
/// Creates the parent directories if they do not exist. The operation has
/// a 5-minute timeout to prevent indefinite hangs on network issues.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the clone fails or times out.
pub fn clone_repository(target: &Utf8Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = run_git_with_timeout(
        &["clone", WHITAKER_REPO_URL, target.as_str()],
        None,
        "clone",
    )?;

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
/// Runs `git pull` in the specified repository directory. The operation has
/// a 5-minute timeout to prevent indefinite hangs on network issues.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the pull fails or times out.
pub fn update_repository(repo: &Utf8Path) -> Result<()> {
    let output = run_git_with_timeout(&["pull"], Some(repo), "pull")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "pull",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
}

/// Runs a git command with a timeout.
///
/// Returns the command output if it completes within the timeout, or an error
/// if the command times out or fails to start.
fn run_git_with_timeout(
    args: &[&str],
    working_dir: Option<&Utf8Path>,
    operation: &'static str,
) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir.as_std_path());
    }

    let mut child = cmd.spawn()?;

    match child.wait_timeout(GIT_TIMEOUT)? {
        Some(status) => {
            // Command completed within timeout - collect output
            let stdout = child
                .stdout
                .take()
                .map(std::io::read_to_string)
                .transpose()?
                .unwrap_or_default();
            let stderr = child
                .stderr
                .take()
                .map(std::io::read_to_string)
                .transpose()?
                .unwrap_or_default();

            Ok(Output {
                status,
                stdout: stdout.into_bytes(),
                stderr: stderr.into_bytes(),
            })
        }
        None => {
            // Timeout - kill the process
            let _ = child.kill();
            let _ = child.wait();
            Err(InstallerError::Git {
                operation,
                message: format!(
                    "operation timed out after {} seconds",
                    GIT_TIMEOUT.as_secs()
                ),
            })
        }
    }
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
