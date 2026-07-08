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
    let output =
        run_git_with_timeout(&["fetch", "origin", refspec, "--tags"], Some(repo), "fetch")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "fetch",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
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
    let output = run_git_with_timeout(&["checkout", "--detach", commit], Some(repo), "checkout")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "checkout",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
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
    let output = run_git_with_timeout(&["checkout", &branch], Some(repo), "checkout")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "checkout",
            message: stderr.trim().to_owned(),
        });
    }

    Ok(())
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
    use camino::Utf8PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    /// Run a git command in `dir`, asserting success, and return trimmed stdout.
    fn git(dir: &Utf8Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir.as_std_path())
            .output()
            .expect("failed to spawn git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Commit the given file content in `dir` and return the resulting SHA.
    fn commit_file(dir: &Utf8Path, name: &str, contents: &str, message: &str) -> String {
        std::fs::write(dir.join(name).as_std_path(), contents).expect("write fixture file");
        git(dir, &["add", "."]);
        git(
            dir,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                message,
            ],
        );
        git(dir, &["rev-parse", "HEAD"])
    }

    /// A source repository plus a clone of it, with recorded commit SHAs.
    struct GitFixture {
        _source: TempDir,
        _clone: TempDir,
        clone: Utf8PathBuf,
        first: String,
        second: String,
    }

    /// Build a source repo (two commits, tag `v1` on the first) and clone it.
    fn git_fixture() -> GitFixture {
        let source = TempDir::new().expect("source temp dir");
        let source_path =
            Utf8PathBuf::try_from(source.path().to_owned()).expect("UTF-8 source path");
        git(&source_path, &["init", "-b", "main"]);
        let first = commit_file(&source_path, "a.txt", "one", "first");
        git(&source_path, &["tag", "v1"]);
        let second = commit_file(&source_path, "b.txt", "two", "second");

        let clone = TempDir::new().expect("clone temp dir");
        let clone_path = Utf8PathBuf::try_from(clone.path().to_owned()).expect("UTF-8 clone path");
        let output = Command::new("git")
            .args(["clone", source_path.as_str(), clone_path.as_str()])
            .output()
            .expect("failed to spawn git clone");
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        GitFixture {
            _source: source,
            _clone: clone,
            clone: clone_path,
            first,
            second,
        }
    }

    #[test]
    fn resolve_commit_resolves_tag_branch_and_sha() {
        let fx = git_fixture();
        assert_eq!(
            resolve_commit(&fx.clone, "v1").expect("resolve tag"),
            fx.first
        );
        assert_eq!(
            resolve_commit(&fx.clone, "main").expect("resolve branch"),
            fx.second
        );
        assert_eq!(
            resolve_commit(&fx.clone, &fx.second).expect("resolve sha"),
            fx.second
        );
    }

    #[test]
    fn resolve_commit_errors_on_garbage() {
        let fx = git_fixture();
        let err = resolve_commit(&fx.clone, "definitely-not-a-ref").expect_err("expected error");
        assert!(matches!(err, InstallerError::Git { .. }), "got {err:?}");
    }

    #[test]
    fn checkout_detached_leaves_head_at_commit() {
        let fx = git_fixture();
        checkout_detached(&fx.clone, &fx.first).expect("checkout detached");
        assert_eq!(git(&fx.clone, &["rev-parse", "HEAD"]), fx.first);
        // A detached HEAD has no symbolic ref.
        let symbolic = Command::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(fx.clone.as_std_path())
            .output()
            .expect("spawn symbolic-ref");
        assert!(!symbolic.status.success(), "expected detached HEAD");
    }

    #[test]
    fn ensure_default_branch_reattaches_so_update_succeeds() {
        let fx = git_fixture();
        checkout_detached(&fx.clone, &fx.first).expect("checkout detached");
        ensure_default_branch(&fx.clone).expect("reattach to default branch");
        assert_eq!(git(&fx.clone, &["symbolic-ref", "HEAD"]), "refs/heads/main");
        // A pull now succeeds because HEAD is on a branch again.
        update_repository(&fx.clone).expect("update after reattach");
    }

    #[test]
    fn ensure_default_branch_is_noop_on_a_branch() {
        let fx = git_fixture();
        ensure_default_branch(&fx.clone).expect("noop on branch");
        assert_eq!(git(&fx.clone, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    }

    #[test]
    fn fetch_ref_retrieves_a_new_tag() {
        let fx = git_fixture();
        // Add a third commit and tag it in the source, after the clone was made.
        let source = Utf8PathBuf::try_from(fx._source.path().to_owned()).expect("UTF-8 path");
        let third = commit_file(&source, "c.txt", "three", "third");
        git(&source, &["tag", "v2"]);

        // The clone cannot resolve the new tag until it fetches.
        assert!(resolve_commit(&fx.clone, "v2").is_err());
        fetch_ref(&fx.clone, "v2").expect("fetch new tag");
        assert_eq!(resolve_commit(&fx.clone, "v2").expect("resolve v2"), third);
    }

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
