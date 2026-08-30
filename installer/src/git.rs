//! Git operations for cloning and updating the Whitaker repository.
//!
//! This module provides functions for managing the local Whitaker clone,
//! including initial cloning and subsequent updates. Operations have a
//! configurable timeout to prevent hangs on network issues.

use std::{
    io::Read,
    process::{Child, Command, Output, Stdio},
    thread::JoinHandle,
    time::Duration,
};

use camino::Utf8Path;
use wait_timeout::ChildExt;

use crate::{
    error::{InstallerError, Result},
    workspace::WHITAKER_REPO_URL,
};

/// Default timeout for git operations (5 minutes).
const GIT_TIMEOUT: Duration = Duration::from_mins(5);

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
    let mut child = spawn_git_child(args, working_dir)?;

    // Take ownership of pipes before spawning threads to avoid blocking.
    // If either pipe is missing, use empty readers.
    let stdout_reader = spawn_pipe_reader(child.stdout.take());
    let stderr_reader = spawn_pipe_reader(child.stderr.take());

    let Some(status) = child.wait_timeout(GIT_TIMEOUT)? else {
        return Err(abandon_timed_out_git(
            child,
            stdout_reader,
            stderr_reader,
            operation,
        ));
    };

    Ok(Output {
        status,
        stdout: join_pipe_reader(stdout_reader, operation, "stdout")?.into_bytes(),
        stderr: join_pipe_reader(stderr_reader, operation, "stderr")?.into_bytes(),
    })
}

/// Spawns the git child process with both standard streams piped.
fn spawn_git_child(args: &[&str], working_dir: Option<&Utf8Path>) -> Result<Child> {
    let mut cmd = Command::new("git");
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir.as_std_path());
    }

    Ok(cmd.spawn()?)
}

/// Handle for a thread draining one of the child's output pipes.
type PipeReader = JoinHandle<std::io::Result<String>>;

/// Drains an optional child pipe on its own thread, yielding an empty string
/// when the pipe is absent.
fn spawn_pipe_reader<R>(pipe: Option<R>) -> PipeReader
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        pipe.map(std::io::read_to_string)
            .transpose()
            .map(Option::unwrap_or_default)
    })
}

/// Joins a pipe reader, reporting a git error when the reader thread panicked.
fn join_pipe_reader(reader: PipeReader, operation: &'static str, stream: &str) -> Result<String> {
    Ok(reader
        .join()
        .map_err(|_| InstallerError::Git {
            operation,
            message: format!("failed to read {stream}"),
        })?
        .unwrap_or_default())
}

/// Kills a timed-out git child and reaps its reader threads before reporting.
///
/// Each cleanup result is discarded deliberately: the operation has already
/// failed, so cleanup is best effort.
fn abandon_timed_out_git(
    mut child: Child,
    stdout_reader: PipeReader,
    stderr_reader: PipeReader,
    operation: &'static str,
) -> InstallerError {
    drop(child.kill());
    drop(child.wait());
    drop(stdout_reader.join());
    drop(stderr_reader.join());
    InstallerError::Git {
        operation,
        message: format!(
            "operation timed out after {} seconds",
            GIT_TIMEOUT.as_secs()
        ),
    }
}

#[cfg(test)]
mod tests {
    //! Tests for git clone and update helpers.

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
