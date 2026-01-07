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
///
/// Spawns threads to read stdout and stderr concurrently to avoid potential
/// deadlocks if the child process produces large output that fills OS buffers.
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

    // Take ownership of pipes before spawning threads to avoid blocking.
    // If either pipe is missing, use empty readers.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    // Spawn threads to read pipes concurrently whilst the process runs.
    let stdout_thread = std::thread::spawn(move || -> std::io::Result<String> {
        stdout_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(|opt| opt.unwrap_or_default())
    });
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<String> {
        stderr_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(|opt| opt.unwrap_or_default())
    });

    match child.wait_timeout(GIT_TIMEOUT)? {
        Some(status) => {
            // Command completed within timeout - collect output from threads
            let stdout = stdout_thread
                .join()
                .map_err(|_| InstallerError::Git {
                    operation,
                    message: "failed to read stdout".to_owned(),
                })?
                .unwrap_or_default();
            let stderr = stderr_thread
                .join()
                .map_err(|_| InstallerError::Git {
                    operation,
                    message: "failed to read stderr".to_owned(),
                })?
                .unwrap_or_default();

            Ok(Output {
                status,
                stdout: stdout.into_bytes(),
                stderr: stderr.into_bytes(),
            })
        }
        None => {
            // Timeout - kill the process and wait for threads to finish
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
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
