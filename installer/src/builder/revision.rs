//! Suite revision provenance for source builds.
//!
//! A source build compiles the lint suite from the checkout the caller named,
//! so it can report which commit produced the binary. The probe reads that
//! checkout's `HEAD` and marks a worktree with uncommitted source, and the
//! value reaches the `cargo build` child through `WHITAKER_SUITE_REVISION`.
//!
//! Process execution is behind [`SuiteRevisionProbe`] rather than called
//! directly from the build path, so a test can substitute a probe that fails or
//! returns a known revision. [`GitSuiteRevisionProbe`] is the production
//! implementation; it pins Git to the named directory, because `-C` alone
//! cannot do that: repository-selection variables are applied after it and
//! outrank it.

use std::{io, process::Command};

use camino::Utf8Path;
use thiserror::Error;

use crate::git::clear_repository_selectors;

/// Why a checkout's suite revision could not be resolved.
///
/// The variants keep the causes apart: a directory that is not a repository is
/// a normal source build, while a Git that cannot be spawned or a probe that
/// fails is an environment problem worth reporting.
#[derive(Debug, Error)]
pub(crate) enum ProbeError {
    /// Git could not be spawned at all.
    #[error("could not run git: {0}")]
    Spawn(#[from] io::Error),
    /// Git ran but rejected the probe, as it does outside a repository.
    #[error("`git {args}` failed in {workspace_root}: {stderr}")]
    Command {
        /// The subcommand that failed, for the report.
        args: String,
        /// The checkout the probe ran in.
        workspace_root: String,
        /// Git's own explanation, trimmed for a single log line.
        stderr: String,
    },
    /// Git succeeded but its output was not the UTF-8 it always emits here.
    #[error("`git {args}` produced output that is not UTF-8")]
    NotUtf8 {
        /// The subcommand whose output could not be decoded.
        args: String,
    },
}

/// Resolve the suite revision of the checkout a build compiles.
///
/// Implementations report the source checkout's `HEAD`, marked when the
/// worktree carries uncommitted source. Returning `Ok(None)` means the
/// directory is not a Git repository, which is not an error: the build still
/// works and its diagnostics report `"unknown"` provenance.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SuiteRevisionProbe {
    /// Probe `workspace_root` for the revision to embed.
    ///
    /// # Errors
    ///
    /// Returns [`ProbeError`] when Git cannot run or a probe fails for an
    /// environmental reason. A directory that is simply not a repository is
    /// `Ok(None)`.
    fn probe(&self, workspace_root: &Utf8Path) -> Result<Option<String>, ProbeError>;
}

/// Probe the checkout by running Git in it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct GitSuiteRevisionProbe;

impl SuiteRevisionProbe for GitSuiteRevisionProbe {
    fn probe(&self, workspace_root: &Utf8Path) -> Result<Option<String>, ProbeError> {
        let Some(head) = probe_git(workspace_root, &["rev-parse", "--short", "HEAD"])? else {
            return Ok(None);
        };
        let sha = head.trim();
        if sha.is_empty() {
            return Ok(None);
        }

        // Ask for untracked files explicitly: a new module is uncommitted
        // source the build compiles, but `status.showUntrackedFiles` can hide
        // it and leave a dirty checkout labelled with a clean revision.
        let status = probe_git(
            workspace_root,
            &["status", "--porcelain", "--untracked-files=normal"],
        )?;
        Ok(status.map(|porcelain| format_suite_revision(sha, !porcelain.trim().is_empty())))
    }
}

/// Run one Git probe, distinguishing "not a repository" from a real failure.
///
/// A repository that Git reports as missing is `Ok(None)`. Any other non-zero
/// exit is a [`ProbeError::Command`], so an unexpected failure is reported
/// rather than silently treated as absent provenance.
fn probe_git(workspace_root: &Utf8Path, args: &[&str]) -> Result<Option<String>, ProbeError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(workspace_root.as_str()).args(args);
    clear_repository_selectors(&mut command);

    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if is_not_a_repository(&stderr) {
            return Ok(None);
        }
        return Err(ProbeError::Command {
            args: args.join(" "),
            workspace_root: workspace_root.to_string(),
            stderr,
        });
    }
    String::from_utf8(output.stdout)
        .map(Some)
        .map_err(|_| ProbeError::NotUtf8 {
            args: args.join(" "),
        })
}

/// Report whether Git's complaint means the directory is not a repository.
fn is_not_a_repository(stderr: &str) -> bool { stderr.contains("not a git repository") }

/// Publish a probed suite revision to a build command.
///
/// A revision that could not be resolved clears the variable instead of
/// leaving it alone. The parent environment is not proof of this checkout's
/// provenance: an inherited value would label the build with another
/// checkout's revision.
pub(crate) fn apply_suite_revision(cmd: &mut Command, revision: Option<&str>) {
    match revision {
        Some(probed) => {
            cmd.env("WHITAKER_SUITE_REVISION", probed);
        }
        None => {
            cmd.env_remove("WHITAKER_SUITE_REVISION");
        }
    }
}

/// Resolve the revision to embed, reporting a failed probe and continuing.
///
/// A probe failure is logged rather than failing the build: the lint still
/// works, and its warnings report `"unknown"` provenance instead of preventing
/// a source build outright.
pub(crate) fn resolve_revision(
    probe: &dyn SuiteRevisionProbe,
    workspace_root: &Utf8Path,
) -> Option<String> {
    match probe.probe(workspace_root) {
        Ok(revision) => revision,
        Err(error) => {
            tracing::warn!(
                %error,
                workspace_root = %workspace_root,
                "could not resolve the suite revision; the build will report unknown provenance"
            );
            None
        }
    }
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
