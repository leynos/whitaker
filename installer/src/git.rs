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

use std::process::Output;

use camino::Utf8Path;
use command::run_git_with_timeout;
pub(crate) use command::{GitInvocation, run_git};
pub use commit_sha::CommitSha;
use tracing::{debug, warn};

use crate::{
    artefact::suite_ref::SuiteRef,
    error::{InstallerError, Result},
    workspace::WHITAKER_REPO_URL,
};

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

/// Brings an existing Whitaker clone level with the remote's default branch.
///
/// Fetches `origin`, refreshes the cached `refs/remotes/origin/HEAD` with
/// `git remote set-head origin --auto`, and hard-resets to it. The refresh
/// matters because a fetch never retargets `origin/HEAD`: a clone that predates
/// an upstream default-branch rename would otherwise keep resetting to the old
/// branch. The branch comes from the remote rather than being assumed to be
/// `main`.
///
/// The clone is a build input the installer manages, not somewhere work
/// happens, so it must not be able to stop an update. A plain `git pull` did: a
/// clone restored from an older CI cache can carry untracked files that the
/// suite has since started to track, and `pull` aborts on them with "untracked
/// working tree files would be overwritten by merge". `reset --hard` overwrites
/// such files, discards any local edit to a tracked file, and leaves the
/// checked-out branch at the remote tip. Each Git subprocess has a 5-minute
/// timeout to prevent indefinite hangs on network issues.
///
/// # Errors
///
/// Returns `InstallerError::Git`, naming the failed step (`fetch`, `set-head`
/// or `reset`) and carrying Git's trimmed diagnostic, if a step fails or times
/// out.
pub fn update_repository(repo: &Utf8Path) -> Result<()> {
    run_git_checked(&["fetch", "origin"], repo, "fetch")?;
    run_git_checked(
        &["remote", "set-head", "origin", "--auto"],
        repo,
        "set-head",
    )?;
    run_git_checked(
        &["reset", "--hard", "refs/remotes/origin/HEAD"],
        repo,
        "reset",
    )?;
    Ok(())
}

/// Runs one Git operation in `repo`, mapping a non-zero exit to an error.
///
/// # Errors
///
/// Returns `InstallerError::Git` if git cannot be run, times out, or exits
/// unsuccessfully, carrying the trimmed standard error.
fn run_git_checked(
    args: &[&str],
    repo: &Utf8Path,
    operation: &'static str,
) -> Result<std::process::Output> {
    let output = run_git_with_timeout(args, Some(repo), operation)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::Git {
            operation,
            message: stderr.trim().to_owned(),
        });
    }
    Ok(output)
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
        log_restore_failure();
        return Err(git_failure(&head, "head"));
    }

    let branch = default_branch_name(&head);
    let output = run_git_with_timeout(&["checkout", "--force", &branch], Some(repo), "checkout")?;
    if output.status.success() {
        return Ok(());
    }
    log_restore_failure();
    Err(git_failure(&output, "checkout"))
}

/// Report a failed attempt to restore the clone's default branch.
///
/// Both failing steps report the same transition, so the message names the
/// transition rather than the step; the accompanying error carries the step.
fn log_restore_failure() {
    warn!(
        operation = "restore_default_branch",
        transition = "detached_to_default",
        outcome = "failure"
    );
}

/// Extract the local branch name from `origin`'s default branch reference.
///
/// Only the remote name is removed: a default branch may itself contain
/// slashes, such as `release/stable`.
fn default_branch_name(head: &Output) -> String {
    let remote_branch = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    remote_branch
        .strip_prefix("origin/")
        .unwrap_or(&remote_branch)
        .to_owned()
}

/// Build the error a failed Git invocation should surface.
fn git_failure(output: &Output, operation: &'static str) -> InstallerError {
    InstallerError::Git {
        operation,
        message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
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
    let commit = resolve_or_fetch(repo, reference)?;
    checkout_detached(repo, &commit)
}

/// Select the commit for `reference`, fetching it when the clone lacks it.
fn resolve_or_fetch(repo: &Utf8Path, reference: &SuiteRef) -> Result<String> {
    if let Some(commit) = resolve_commit(repo, reference)? {
        log_reference_resolved("local", "resolved the requested suite reference locally");
        return Ok(commit);
    }
    log_resolution_event(
        "origin",
        "fetch_fallback",
        "requested suite reference was unavailable locally",
    );
    fetch_unavailable_ref(repo, reference)
}

/// Report that a suite reference resolved from a named source.
fn log_reference_resolved(source: &'static str, message: &'static str) {
    log_resolution_event(source, "resolved", message);
}

/// Report one step of the suite-reference resolution sequence.
///
/// `source` names where the reference was found (or sought) and `outcome`
/// records what happened there, so operators can follow the fallback order
/// without reading the message.
fn log_resolution_event(source: &'static str, outcome: &'static str, message: &'static str) {
    debug!(
        operation = "checkout_ref",
        resolution_source = source,
        outcome,
        "{message}"
    );
}

/// Fetches a reference unavailable in the clone and selects its commit.
fn fetch_unavailable_ref(repo: &Utf8Path, reference: &SuiteRef) -> Result<String> {
    fetch_all_refs(repo)?;
    if let Some(commit) = resolve_commit(repo, reference)? {
        log_reference_resolved(
            "origin",
            "resolved the requested suite reference after fetching origin",
        );
        return Ok(commit);
    }
    fetch_named_ref(repo, reference)
}

/// Fetch every branch and tag `origin` advertises.
fn fetch_all_refs(repo: &Utf8Path) -> Result<()> {
    let fetch = run_git_with_timeout(
        &["fetch", "--tags", "--force", "origin"],
        Some(repo),
        "fetch",
    )?;
    if fetch.status.success() {
        return Ok(());
    }
    log_resolution_failure("origin", "could not fetch suite references from origin");
    Err(git_failure(&fetch, "fetch"))
}

/// Report a fetch that failed while resolving the requested reference.
fn log_resolution_failure(source: &'static str, message: &'static str) {
    warn!(
        operation = "checkout_ref",
        resolution_source = source,
        outcome = "failure",
        "{message}"
    );
}

/// Fetch one reference by name, reaching a commit no branch or tag contains.
fn fetch_named_ref(repo: &Utf8Path, reference: &SuiteRef) -> Result<String> {
    let targeted = run_git_with_timeout(
        &["fetch", "--force", "origin", reference.as_str()],
        Some(repo),
        "fetch",
    )?;
    if !targeted.status.success() {
        log_resolution_failure(
            "targeted_origin",
            "could not fetch the requested suite reference from origin",
        );
        let stderr = String::from_utf8_lossy(&targeted.stderr);
        return Err(InstallerError::Git {
            operation: "fetch",
            message: format!("could not fetch {reference}: {}", stderr.trim()),
        });
    }
    log_reference_resolved(
        "targeted_origin",
        "resolved the requested suite reference through FETCH_HEAD",
    );
    Ok("FETCH_HEAD".to_owned())
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

    if output.status.success() {
        return Ok(());
    }
    log_detached_failure();
    Err(git_failure(&output, "checkout"))
}

/// Report a failed detached checkout of the selected suite revision.
fn log_detached_failure() {
    warn!(
        operation = "checkout_detached",
        transition = "attached_to_detached",
        outcome = "failure"
    );
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
