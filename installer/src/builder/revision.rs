//! Suite revision provenance for source builds.
//!
//! A source build compiles the lint suite from the checkout the caller named,
//! so it can report which commit produced the binary. The probe reads that
//! checkout's `HEAD` and marks a worktree with uncommitted source, and the
//! value reaches the `cargo build` child through `WHITAKER_SUITE_REVISION`.
//!
//! Everything here pins Git to the named directory: `-C` alone cannot do that,
//! because repository-selection variables are applied after it and outrank it.

use std::process::Command;

use camino::Utf8Path;

use crate::git::clear_repository_selectors;

/// Publish a probed suite revision to a build command.
///
/// A missing revision clears the variable instead of leaving it alone. The
/// parent environment is not proof of this checkout's provenance: an inherited
/// value would label the build with another checkout's revision.
pub(crate) fn apply_suite_revision(cmd: &mut Command, revision: Option<String>) {
    match revision {
        Some(probed) => {
            cmd.env("WHITAKER_SUITE_REVISION", probed);
        }
        None => {
            cmd.env_remove("WHITAKER_SUITE_REVISION");
        }
    }
}

/// Resolve the source checkout's suite revision for embedding.
///
/// Returns the short `HEAD` commit, suffixed with `+dirty` when the worktree
/// has uncommitted changes. Returns `None` when `git` cannot be run, the
/// directory is not a repository, or either probe fails; the lint then reports
/// its revision as `"unknown"` rather than inventing one.
///
/// The status probe asks for untracked files explicitly. A new module is
/// uncommitted source that the build compiles, but `status.showUntrackedFiles`
/// can hide it, leaving a dirty checkout labelled with a clean revision.
pub(crate) fn suite_revision(workspace_root: &Utf8Path) -> Option<String> {
    let head = git_output(workspace_root, &["rev-parse", "--short", "HEAD"])?;
    let sha = head.trim();
    if sha.is_empty() {
        return None;
    }
    let status = git_output(
        workspace_root,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    Some(format_suite_revision(sha, !status.trim().is_empty()))
}

/// Run one Git probe in the checkout and return its standard output.
fn git_output(workspace_root: &Utf8Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(workspace_root.as_str()).args(args);
    clear_repository_selectors(&mut command);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Format an embedded revision, marking uncommitted source as `+dirty`.
fn format_suite_revision(sha: &str, is_dirty: bool) -> String {
    if is_dirty {
        format!("{sha}+dirty")
    } else {
        sha.to_owned()
    }
}

#[cfg(test)]
#[path = "../builder_revision_tests.rs"]
mod tests;
