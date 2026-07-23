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

    run_git_checked(
        &["clone", WHITAKER_REPO_URL, target.as_str()],
        None,
        "clone",
    )
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
    run_git_checked(&["pull"], Some(repo), "pull")
}

/// Resolves a commit-ish (SHA, tag, or branch) to a full commit SHA.
///
/// Runs `git rev-parse --verify <refspec>^{commit}` in the repository so that
/// only expressions naming an existing commit succeed; annotated tags are
/// peeled to the commit they point at.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the ref cannot be resolved or the command
/// times out.
pub fn resolve_commit(repo: &Utf8Path, refspec: &str) -> Result<String> {
    let peeled = format!("{refspec}^{{commit}}");
    let output =
        run_git_with_timeout(&["rev-parse", "--verify", &peeled], Some(repo), "rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "rev-parse",
            message: format!("could not resolve ref '{refspec}': {}", stderr.trim()),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Fetches a specific ref (and all tags) from `origin` into the repository.
///
/// Used to recover when a pinned ref cannot be resolved from the existing
/// clone. Runs `git fetch origin <refspec> --tags`.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the fetch fails or times out.
pub fn fetch_ref(repo: &Utf8Path, refspec: &str) -> Result<()> {
    run_git_checked(&["fetch", "origin", refspec, "--tags"], Some(repo), "fetch")
}

/// Checks out a commit as a detached HEAD.
///
/// Runs `git checkout --detach <commit>`, leaving the working tree at exactly
/// the given commit without moving any branch.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the checkout fails or times out.
pub fn checkout_detached(repo: &Utf8Path, commit: &str) -> Result<()> {
    run_git_checked(&["checkout", "--detach", commit], Some(repo), "checkout")
}

/// Reattaches the repository to its default branch when HEAD is detached.
///
/// A previous pinned install may leave the platform clone on a detached HEAD,
/// which makes a later `git pull` fail. This restores a branch checkout so that
/// subsequent updates succeed. It is a no-op when HEAD is already on a branch.
///
/// The default branch is discovered from `origin/HEAD`
/// (`git rev-parse --abbrev-ref origin/HEAD`, e.g. `origin/main`); when an older
/// clone lacks that symbolic ref, `git remote set-head origin --auto` restores
/// it before retrying.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the default branch cannot be determined or
/// checked out.
pub fn ensure_default_branch(repo: &Utf8Path) -> Result<()> {
    // When HEAD already names a branch, there is nothing to reattach.
    let symbolic =
        run_git_with_timeout(&["symbolic-ref", "-q", "HEAD"], Some(repo), "symbolic-ref")?;
    if symbolic.status.success() {
        return Ok(());
    }

    let branch = default_branch_name(repo)?;
    run_git_checked(&["checkout", &branch], Some(repo), "checkout")
}

/// Discovers the remote default branch name (without the `origin/` prefix).
fn default_branch_name(repo: &Utf8Path) -> Result<String> {
    if let Some(branch) = read_default_branch(repo)? {
        return Ok(branch);
    }

    // An older clone may lack origin/HEAD; ask git to repopulate it, then retry.
    let _ = run_git_with_timeout(
        &["remote", "set-head", "origin", "--auto"],
        Some(repo),
        "remote",
    )?;

    read_default_branch(repo)?.ok_or_else(|| InstallerError::Git {
        operation: "rev-parse",
        message: "could not determine default branch from origin/HEAD".to_owned(),
    })
}

/// Reads `origin/HEAD` and returns the bare branch name, if present.
fn read_default_branch(repo: &Utf8Path) -> Result<Option<String>> {
    let output = run_git_with_timeout(
        &["rev-parse", "--abbrev-ref", "origin/HEAD"],
        Some(repo),
        "rev-parse",
    )?;
    if !output.status.success() {
        return Ok(None);
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(value
        .strip_prefix("origin/")
        .map(ToOwned::to_owned)
        .filter(|branch| !branch.is_empty()))
}

/// Runs a Git command whose successful output is intentionally discarded.
fn run_git_checked(
    args: &[&str],
    working_dir: Option<&Utf8Path>,
    operation: &'static str,
) -> Result<()> {
    let output = run_git_with_timeout(args, working_dir, operation)?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(InstallerError::Git {
        operation,
        message: stderr.trim().to_owned(),
    })
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
#[path = "git_tests.rs"]
mod tests;
