//! Produce the published snapshot bundle from upstream Clippy.
//!
//! This is the one place Whitaker reads public Clippy, and it runs only on
//! the producer: a release job, or a CI cache miss. It clones `main`, proves
//! the pinned commit is on `main`'s first-parent history, and bundles exactly
//! `refs/heads/main` (moved to the pinned commit) and `HEAD`. The bundle is
//! then installed into a scratch root through [`install_bundle`], so a bundle
//! is only published after the consumer's own checks have accepted it.

use super::error::ClippySnapshotError;
use super::git::SnapshotGit;
use super::install::{BundleArtefact, absolute, install_bundle};
use super::manifest::{ClippySnapshotManifest, SNAPSHOT_BRANCH};
use crate::artefact::packaging::compute_sha256;
use camino::{Utf8Path, Utf8PathBuf};
use std::time::Duration;

/// Attempts at the upstream clone before giving up.
const CLONE_ATTEMPTS: u32 = 3;

/// The URL-scoped config key that confines the retry token to GitHub.
const GITHUB_HEADER_KEY: &str = "http.https://github.com/.extraHeader";

/// Base delay between attempts; attempt `n` waits `n` times this.
const CLONE_BACKOFF: Duration = Duration::from_secs(5);

/// Inputs for one producer build.
#[derive(Debug, Clone)]
pub struct BuildRequest<'a> {
    /// The snapshot to produce.
    pub manifest: &'a ClippySnapshotManifest,
    /// The repository to clone: the upstream URL, or a local mirror.
    pub source: &'a str,
    /// Scratch space for the clone; its contents are disposable.
    pub work_dir: &'a Utf8Path,
    /// Where the bundle and sidecar are written.
    pub output_dir: &'a Utf8Path,
    /// A GitHub token for retries after an anonymous clone is refused.
    pub token: Option<&'a str>,
}

/// The files a producer build publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltBundle {
    /// The bundle and its digest.
    pub artefact: BundleArtefact,
    /// The `.sha256` sidecar beside it.
    pub sidecar: Utf8PathBuf,
}

/// Build, self-verify, and write the snapshot bundle and sidecar.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Git`] when the upstream clone fails after
/// every attempt, [`ClippySnapshotError::Invalid`] when the pinned commit is
/// not on `main`'s first-parent history, and any error [`install_bundle`]
/// raises when the result fails the consumer's checks.
pub fn build_bundle(request: &BuildRequest<'_>) -> Result<BuiltBundle, ClippySnapshotError> {
    // Git runs inside the mirror below, so the caller's relative directories
    // must be fixed against the caller's directory first.
    let work_dir = absolute(request.work_dir)?;
    let output_dir = absolute(request.output_dir)?;
    let request = &BuildRequest {
        work_dir: &work_dir,
        output_dir: &output_dir,
        ..request.clone()
    };
    let mirror = request.work_dir.join("upstream.git");
    clone_upstream(request, &mirror)?;
    pin_main(&mirror, request.manifest)?;
    std::fs::create_dir_all(request.output_dir)
        .map_err(|error| ClippySnapshotError::io(request.output_dir, &error))?;
    let bundle = request
        .output_dir
        .join(request.manifest.bundle_asset_name());
    SnapshotGit::local().stdout(
        "bundle create",
        Some(&mirror),
        &[
            "bundle",
            "create",
            "--quiet",
            bundle.as_str(),
            "refs/heads/main",
            "HEAD",
        ],
    )?;
    let sha256 = compute_sha256(bundle.as_std_path()).map_err(|error| ClippySnapshotError::Io {
        path: bundle.to_string(),
        reason: error.to_string(),
    })?;
    let sidecar = Utf8PathBuf::from(format!("{bundle}.sha256"));
    let line = format!("{sha256}  {}\n", request.manifest.bundle_asset_name());
    std::fs::write(&sidecar, line).map_err(|error| ClippySnapshotError::io(&sidecar, &error))?;
    let artefact = BundleArtefact::from_sidecar(&bundle, request.manifest)?;
    install_bundle(
        &artefact,
        request.manifest,
        &request.work_dir.join("self-check"),
    )?;
    Ok(BuiltBundle { artefact, sidecar })
}

/// Clone `main` from the source, anonymously first.
///
/// GitHub intermittently refuses anonymous clones from shared CI egress
/// ranges. Retrying with the job's token avoids that limit, but the token is
/// only offered after an anonymous attempt fails, and only to
/// `https://github.com/` URLs: the header is keyed to that prefix, so a clone
/// of a mirror or any other `--source` never receives it. Git matches the key
/// against the URL it was given, not against hosts it is redirected to.
fn clone_upstream(
    request: &BuildRequest<'_>,
    mirror: &Utf8Path,
) -> Result<(), ClippySnapshotError> {
    let mut last_error = None;
    for attempt in 1..=CLONE_ATTEMPTS {
        remove_partial_clone(mirror)?;
        let header = (attempt > 1)
            .then_some(request.token)
            .flatten()
            .map(|token| format!("Authorization: Bearer {token}"));
        // `GIT_CONFIG_*` scopes the header to this one command, and the URL in
        // the key scopes it to GitHub.
        let envs: Vec<(&str, &str)> = header.as_deref().map_or_else(Vec::new, |value| {
            vec![
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", GITHUB_HEADER_KEY),
                ("GIT_CONFIG_VALUE_0", value),
            ]
        });
        let args = [
            "clone",
            "--quiet",
            "--bare",
            "--single-branch",
            "--no-tags",
            "--branch",
            SNAPSHOT_BRANCH,
            request.source,
            mirror.as_str(),
        ];
        match SnapshotGit::network()
            .with_env(&envs)
            .stdout("clone", None, &args)
        {
            Ok(_) => return Ok(()),
            Err(error) => {
                tracing::warn!(attempt, error = %error, "Clippy clone attempt failed");
                last_error = Some(error);
                if attempt < CLONE_ATTEMPTS {
                    std::thread::sleep(CLONE_BACKOFF * attempt);
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| ClippySnapshotError::Git {
        operation: "clone",
        message: "no clone attempt ran".to_owned(),
    }))
}

/// Discard a half-written clone from an earlier failed attempt.
fn remove_partial_clone(mirror: &Utf8Path) -> Result<(), ClippySnapshotError> {
    match std::fs::remove_dir_all(mirror) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ClippySnapshotError::io(mirror, &error)),
    }
}

/// Point `main` at the pinned commit after proving it belongs there.
fn pin_main(
    mirror: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<(), ClippySnapshotError> {
    let git = SnapshotGit::local();
    let commit = manifest.commit.as_str();
    let first_parents = git.stdout(
        "rev-list",
        Some(mirror),
        &["rev-list", "--first-parent", "refs/heads/main"],
    )?;
    if !first_parents.lines().any(|line| line == commit) {
        return Err(ClippySnapshotError::invalid(
            mirror,
            format!("the pinned commit {commit} is not on main's first-parent history"),
        ));
    }
    git.stdout(
        "update-ref",
        Some(mirror),
        &["update-ref", "refs/heads/main", commit],
    )?;
    git.stdout(
        "symbolic-ref",
        Some(mirror),
        &["symbolic-ref", "HEAD", "refs/heads/main"],
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
