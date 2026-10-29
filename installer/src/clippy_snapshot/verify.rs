//! Integrity checks for an installed Clippy snapshot repository.
//!
//! [`verify_repository`] is the full check run before a generation is
//! accepted and before a cold driver build reads it. [`check_installed`] is
//! the cheap subset a warm `whitaker` run repeats every time: it proves the
//! generation is still the pinned one without re-hashing the object database.

use super::error::ClippySnapshotError;
use super::generation::GenerationMetadata;
use super::git::SnapshotGit;
use super::manifest::{ClippySnapshotManifest, SNAPSHOT_BRANCH};
use super::probe::{is_present, is_present_dir};
use super::selection::{channel_at, select_revision, toolchain_changes};
use super::telemetry::{Operation, timed};
use camino::Utf8Path;

/// The only ref a snapshot may carry.
fn main_ref() -> String {
    format!("refs/heads/{SNAPSHOT_BRANCH}")
}

/// Prove that `path` holds exactly the history `manifest` pins.
///
/// The checks run cheapest first and each failure names the check, so a
/// broken snapshot says what is wrong rather than that something is.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::NotInstalled`] when `path` is absent,
/// [`ClippySnapshotError::InsufficientHistory`] when Dylint's walk could not
/// reach the pinned revision, and [`ClippySnapshotError::Invalid`] for any
/// other failed check.
pub fn verify_repository(
    path: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<(), ClippySnapshotError> {
    timed(Operation::Verify, || {
        let git = SnapshotGit::local();
        check_structure(git, path, manifest.commit.as_str())?;
        check_self_contained(git, path)?;
        check_objects(git, path)?;
        check_selection(git, path, manifest)
    })
}

/// Prove that an installed generation is still the one its metadata names.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::NotInstalled`] when `path` is absent and
/// [`ClippySnapshotError::Invalid`] when the refs or metadata disagree.
pub fn check_installed(path: &Utf8Path, commit: &str) -> Result<(), ClippySnapshotError> {
    let metadata = GenerationMetadata::read(path)?;
    if metadata.commit != commit {
        return Err(ClippySnapshotError::invalid(
            path,
            format!(
                "its metadata records commit {}, expected {commit}",
                metadata.commit
            ),
        ));
    }
    check_structure(SnapshotGit::local(), path, commit)
}

/// Check the repository shape and refs.
fn check_structure(
    git: SnapshotGit<'_>,
    path: &Utf8Path,
    commit: &str,
) -> Result<(), ClippySnapshotError> {
    if !is_present_dir(path)? {
        return Err(ClippySnapshotError::NotInstalled {
            path: path.to_string(),
        });
    }
    let bare = git.stdout(
        "rev-parse",
        Some(path),
        &["rev-parse", "--is-bare-repository"],
    )?;
    if bare != "true" {
        return Err(ClippySnapshotError::invalid(
            path,
            "it is not a bare repository",
        ));
    }
    let head = git.output("symbolic-ref", Some(path), &["symbolic-ref", "HEAD"])?;
    let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    if head != main_ref() {
        return Err(ClippySnapshotError::invalid(
            path,
            format!("HEAD is `{head}`, not a symbolic ref to {}", main_ref()),
        ));
    }
    check_refs(git, path, commit)
}

/// Require exactly one ref, `main`, at the pinned commit.
fn check_refs(
    git: SnapshotGit<'_>,
    path: &Utf8Path,
    commit: &str,
) -> Result<(), ClippySnapshotError> {
    let listing = git.stdout(
        "for-each-ref",
        Some(path),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    )?;
    let refs: Vec<(&str, &str)> = listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .collect();
    let main = main_ref();
    let extra: Vec<&str> = refs
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| *name != main)
        .collect();
    if !extra.is_empty() {
        return Err(ClippySnapshotError::invalid(
            path,
            format!("it carries refs other than {main}: {}", extra.join(", ")),
        ));
    }
    match refs.first() {
        Some((_, object)) if *object == commit => Ok(()),
        Some((_, object)) => Err(ClippySnapshotError::invalid(
            path,
            format!("{main} is {object}, expected the pinned commit {commit}"),
        )),
        None => Err(ClippySnapshotError::invalid(
            path,
            format!("it has no {main}"),
        )),
    }
}

/// Require complete local history with no outside object sources or remotes.
fn check_self_contained(git: SnapshotGit<'_>, path: &Utf8Path) -> Result<(), ClippySnapshotError> {
    let shallow = git.stdout(
        "rev-parse",
        Some(path),
        &["rev-parse", "--is-shallow-repository"],
    )?;
    if shallow != "false" {
        return Err(ClippySnapshotError::invalid(path, "its history is shallow"));
    }
    if is_present(&path.join("objects/info/alternates"))? {
        return Err(ClippySnapshotError::invalid(
            path,
            "it borrows objects through objects/info/alternates",
        ));
    }
    let remotes = git.stdout("remote", Some(path), &["remote"])?;
    if !remotes.is_empty() {
        return Err(ClippySnapshotError::invalid(
            path,
            format!(
                "it has remotes ({}), so it could fetch",
                remotes.replace('\n', ", ")
            ),
        ));
    }
    Ok(())
}

/// Re-hash every object and check connectivity.
fn check_objects(git: SnapshotGit<'_>, path: &Utf8Path) -> Result<(), ClippySnapshotError> {
    let output = git.output(
        "fsck",
        Some(path),
        &[
            "fsck",
            "--full",
            "--strict",
            "--no-dangling",
            "--no-progress",
        ],
    )?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(ClippySnapshotError::invalid(
        path,
        format!("git fsck --full failed: {}", stderr.trim()),
    ))
}

/// Re-run Dylint's selection and require the recorded result.
fn check_selection(
    git: SnapshotGit<'_>,
    path: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<(), ClippySnapshotError> {
    let changes = toolchain_changes(git, path, manifest.commit.as_str())?;
    let selected = select_revision(&changes, &manifest.toolchain).map_err(|gap| {
        ClippySnapshotError::InsufficientHistory {
            path: path.to_string(),
            toolchain: manifest.toolchain.clone(),
            reason: gap.to_string(),
        }
    })?;
    let expected = manifest.selected_revision.as_str();
    if selected.commit != expected {
        return Err(ClippySnapshotError::invalid(
            path,
            format!(
                "Dylint would select {} for {}, but the manifest records {expected}",
                selected.commit, manifest.toolchain
            ),
        ));
    }
    let channel = channel_at(git, path, expected)?;
    if channel.as_deref() != Some(manifest.toolchain.as_str()) {
        return Err(ClippySnapshotError::invalid(
            path,
            format!(
                "the selected revision {expected} names {}, not {}",
                channel.as_deref().unwrap_or("no channel"),
                manifest.toolchain
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
