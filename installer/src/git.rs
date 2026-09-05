//! Git operations for cloning and updating the Whitaker repository.
//!
//! This module provides functions for managing the local Whitaker clone,
//! including initial cloning and subsequent updates. Operations have a
//! configurable timeout to prevent hangs on network issues.

#[path = "git/commit_sha.rs"]
mod commit_sha;

pub use commit_sha::CommitSha;

use crate::artefact::suite_ref::SuiteRef;
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

/// Returns whether the clone's `HEAD` is detached.
///
/// A pinned install leaves it detached, and `git pull` refuses to run there
/// with "You are not currently on a branch", so a later unpinned update has
/// to know before it tries.
///
/// # Errors
///
/// Returns `InstallerError::Git` if git cannot be run or times out.
pub fn is_detached_head(repo: &Utf8Path) -> Result<bool> {
    let output = run_git_with_timeout(&["symbolic-ref", "--quiet", "HEAD"], Some(repo), "head")?;
    // A non-zero status here means HEAD names a commit rather than a branch,
    // which is the question being asked and not a failure.
    Ok(!output.status.success())
}

/// Restores the clone to the remote's default branch.
///
/// Used before updating or reusing a clone that a previous pinned install
/// left detached. The branch comes from `origin/HEAD` rather than being
/// assumed to be `main`, so renaming the default branch upstream does not
/// strand every cached clone.
///
/// # Errors
///
/// Returns `InstallerError::Git` if the default branch cannot be determined
/// or checked out.
pub fn restore_default_branch(repo: &Utf8Path) -> Result<()> {
    let head = run_git_with_timeout(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        Some(repo),
        "head",
    )?;
    if !head.status.success() {
        let stderr = String::from_utf8_lossy(&head.stderr);
        return Err(InstallerError::Git {
            operation: "head",
            message: stderr.trim().to_owned(),
        });
    }
    let remote_branch = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    // `origin/main` names the remote-tracking ref; the local branch to return
    // to is its last component.
    let branch = remote_branch
        .rsplit('/')
        .next()
        .unwrap_or(&remote_branch)
        .to_owned();

    let output = run_git_with_timeout(&["checkout", "--force", &branch], Some(repo), "checkout")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "checkout",
            message: stderr.trim().to_owned(),
        });
    }
    Ok(())
}

/// Resolves a reference to a commit, preferring the remote's view of it.
///
/// Order matters and is the whole point. `git checkout main` resolves the
/// *local* `main`, which a fetch never fast-forwards, so a branch pin would
/// silently build whatever that branch pointed at when the clone was made.
/// Checking `refs/remotes/origin/<ref>` first means a branch pin follows the
/// remote; tags and commits fall through to the later candidates.
fn resolve_commit(repo: &Utf8Path, reference: &SuiteRef) -> Result<Option<String>> {
    let candidates = [
        format!("refs/remotes/origin/{reference}^{{commit}}"),
        format!("refs/tags/{reference}^{{commit}}"),
        format!("{reference}^{{commit}}"),
    ];
    for candidate in &candidates {
        let output = run_git_with_timeout(
            &["rev-parse", "--verify", "--quiet", candidate],
            Some(repo),
            "rev-parse",
        )?;
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !sha.is_empty() {
                return Ok(Some(sha));
            }
        }
    }
    Ok(None)
}

/// Checks out a reference in an existing Whitaker clone.
///
/// Fetches first, so a reference published after the clone was made is
/// available, then resolves it preferring the remote's view and checks out
/// the resulting commit detached. Detached because the checkout is a build
/// input rather than somewhere work happens: landing on a local branch would
/// let a later `pull` move the pinned suite underneath the caller.
///
/// A reference reachable from no branch and no tag, such as a commit on an
/// unmerged branch, is fetched explicitly as a second attempt, because
/// `fetch --tags origin` does not bring down objects no fetched ref reaches.
///
/// The reference is a [`SuiteRef`], which has already refused anything git
/// would reject and anything that would reach the command line as an option
/// rather than as a reference.
///
/// # Errors
///
/// Returns `InstallerError::Git` if a fetch or the checkout fails, if either
/// times out, or if the reference cannot be resolved at all.
pub fn checkout_ref(repo: &Utf8Path, reference: &SuiteRef) -> Result<()> {
    let fetch = run_git_with_timeout(
        &["fetch", "--tags", "--force", "origin"],
        Some(repo),
        "fetch",
    )?;
    if !fetch.status.success() {
        let stderr = String::from_utf8_lossy(&fetch.stderr);
        return Err(InstallerError::Git {
            operation: "fetch",
            message: stderr.trim().to_owned(),
        });
    }

    let commit = match resolve_commit(repo, reference)? {
        Some(commit) => commit,
        None => {
            // Nothing local matches, so ask the remote for this reference by
            // name. This is what reaches a commit that no branch or tag
            // contains, which a caller pinning an exact SHA may well name.
            let targeted = run_git_with_timeout(
                &["fetch", "--force", "origin", reference.as_str()],
                Some(repo),
                "fetch",
            )?;
            if !targeted.status.success() {
                let stderr = String::from_utf8_lossy(&targeted.stderr);
                return Err(InstallerError::Git {
                    operation: "fetch",
                    message: format!("could not fetch {reference}: {}", stderr.trim()),
                });
            }
            "FETCH_HEAD".to_owned()
        }
    };

    let output = run_git_with_timeout(
        &["checkout", "--detach", "--force", &commit],
        Some(repo),
        "checkout",
    )?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "checkout",
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
