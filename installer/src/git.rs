//! Git operations for managed Whitaker clones.
//!
//! The workspace layer uses these helpers to clone and update the suite,
//! resolve pinned references, detach at selected commits, and restore the
//! remote default branch before later unpinned work. Every Git subprocess is
//! bounded by a timeout to prevent network operations from hanging installs.

#[path = "git/command.rs"]
mod command;
#[path = "git/commit_sha.rs"]
mod commit_sha;

pub use commit_sha::CommitSha;

use crate::artefact::suite_ref::SuiteRef;
use crate::error::{InstallerError, Result};
use crate::workspace::WHITAKER_REPO_URL;
use camino::Utf8Path;
use command::run_git_with_timeout;
use tracing::{debug, warn};

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
#[tracing::instrument(
    skip(repo),
    fields(
        operation = "restore_default_branch",
        transition = "detached_to_default"
    )
)]
pub fn restore_default_branch(repo: &Utf8Path) -> Result<()> {
    let head = run_git_with_timeout(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        Some(repo),
        "head",
    )?;
    if !head.status.success() {
        warn!(
            operation = "restore_default_branch",
            transition = "detached_to_default",
            outcome = "failure"
        );
        let stderr = String::from_utf8_lossy(&head.stderr);
        return Err(InstallerError::Git {
            operation: "head",
            message: stderr.trim().to_owned(),
        });
    }
    let remote_branch = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    // Remove only the remote name: default branches may themselves contain
    // slashes, such as `release/stable`.
    let branch = remote_branch
        .strip_prefix("origin/")
        .unwrap_or(&remote_branch)
        .to_owned();

    let output = run_git_with_timeout(&["checkout", "--force", &branch], Some(repo), "checkout")?;
    if !output.status.success() {
        warn!(
            operation = "restore_default_branch",
            transition = "detached_to_default",
            outcome = "failure"
        );
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
/// Resolves locally before fetching, so an existing pin can be checked out
/// offline. On a local miss, it fetches and resolves the reference before
/// checking out the resulting commit detached. Detached because the checkout
/// is a build input rather than somewhere work happens: landing on a local
/// branch would let a later `pull` move the pinned suite underneath the caller.
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
    let commit = match resolve_commit(repo, reference)? {
        Some(commit) => {
            debug!(
                operation = "checkout_ref",
                resolution_source = "local",
                outcome = "resolved",
                "resolved the requested suite reference locally"
            );
            commit
        }
        None => {
            debug!(
                operation = "checkout_ref",
                resolution_source = "origin",
                outcome = "fetch_fallback",
                "requested suite reference was unavailable locally"
            );
            let fetch = run_git_with_timeout(
                &["fetch", "--tags", "--force", "origin"],
                Some(repo),
                "fetch",
            )?;
            if !fetch.status.success() {
                warn!(
                    operation = "checkout_ref",
                    resolution_source = "origin",
                    outcome = "failure",
                    "could not fetch suite references from origin"
                );
                let stderr = String::from_utf8_lossy(&fetch.stderr);
                return Err(InstallerError::Git {
                    operation: "fetch",
                    message: stderr.trim().to_owned(),
                });
            }
            if let Some(commit) = resolve_commit(repo, reference)? {
                debug!(
                    operation = "checkout_ref",
                    resolution_source = "origin",
                    outcome = "resolved",
                    "resolved the requested suite reference after fetching origin"
                );
                return checkout_detached(repo, &commit);
            }
            // Nothing local matches, so ask the remote for this reference by
            // name. This is what reaches a commit that no branch or tag
            // contains, which a caller pinning an exact SHA may well name.
            let targeted = run_git_with_timeout(
                &["fetch", "--force", "origin", reference.as_str()],
                Some(repo),
                "fetch",
            )?;
            if !targeted.status.success() {
                warn!(
                    operation = "checkout_ref",
                    resolution_source = "targeted_origin",
                    outcome = "failure",
                    "could not fetch the requested suite reference from origin"
                );
                let stderr = String::from_utf8_lossy(&targeted.stderr);
                return Err(InstallerError::Git {
                    operation: "fetch",
                    message: format!("could not fetch {reference}: {}", stderr.trim()),
                });
            }
            debug!(
                operation = "checkout_ref",
                resolution_source = "targeted_origin",
                outcome = "resolved",
                "resolved the requested suite reference through FETCH_HEAD"
            );
            "FETCH_HEAD".to_owned()
        }
    };

    checkout_detached(repo, &commit)
}

/// Checks out `commit` detached, preserving the selected suite revision.
#[tracing::instrument(
    skip(repo, commit),
    fields(operation = "checkout_detached", transition = "attached_to_detached")
)]
fn checkout_detached(repo: &Utf8Path, commit: &str) -> Result<()> {
    let output = run_git_with_timeout(
        &["checkout", "--detach", "--force", commit],
        Some(repo),
        "checkout",
    )?;

    if !output.status.success() {
        warn!(
            operation = "checkout_detached",
            transition = "attached_to_detached",
            outcome = "failure"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation: "checkout",
            message: stderr.trim().to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
