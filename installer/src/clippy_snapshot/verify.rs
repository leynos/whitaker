//! Integrity checks for an installed Clippy snapshot repository.
//!
//! [`verify_repository`] is the full check run before a generation is
//! accepted and before a cold driver build reads it. [`check_installed`] is
//! the cheap subset a warm `whitaker` run repeats every time: it proves the
//! generation is still the pinned one without re-hashing the object database.
//!
//! Both entry points compose the production ports and delegate to
//! `*_with` functions. The orchestration only observes through
//! [`RepositoryQueries`] and [`SnapshotStore`]; every accept/reject decision
//! is a pure function in [`super::policy`].

use camino::Utf8Path;

use super::{
    error::ClippySnapshotError,
    manifest::ClippySnapshotManifest,
    policy::{
        SelectionRejection,
        accept_integrity,
        accept_selection,
        accept_self_contained,
        accept_structure,
    },
    ports::{RepositoryQueries, SnapshotStore, SnapshotSupport},
    selection::{channel_at, select_revision, toolchain_changes},
};

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
#[tracing::instrument(name = "clippy_snapshot.verify", skip_all, fields(commit = %manifest.commit))]
pub fn verify_repository(
    path: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<(), ClippySnapshotError> {
    verify_with(path, manifest, &SnapshotSupport::production())
}

/// Prove that `path` holds the pinned history using injected ports.
pub(crate) fn verify_with(
    path: &Utf8Path,
    manifest: &ClippySnapshotManifest,
    support: &SnapshotSupport<'_>,
) -> Result<(), ClippySnapshotError> {
    check_structure(
        path,
        manifest.commit.as_str(),
        support.store,
        support.repository,
    )?;
    check_self_contained(path, support.store, support.repository)?;
    check_objects(path, support.repository)?;
    check_selection(path, manifest, support.repository)
}

/// Prove that an installed generation is still the one its metadata names.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::NotInstalled`] when `path` is absent and
/// [`ClippySnapshotError::Invalid`] when the refs or metadata disagree.
pub fn check_installed(path: &Utf8Path, commit: &str) -> Result<(), ClippySnapshotError> {
    check_installed_with(path, commit, &SnapshotSupport::production())
}

/// The warm path using injected ports.
pub(crate) fn check_installed_with(
    path: &Utf8Path,
    commit: &str,
    support: &SnapshotSupport<'_>,
) -> Result<(), ClippySnapshotError> {
    let metadata = support.metadata.read(path)?;
    if metadata.commit != commit {
        return Err(ClippySnapshotError::invalid(
            path,
            format!(
                "its metadata records commit {}, expected {commit}",
                metadata.commit
            ),
        ));
    }
    check_structure(path, commit, support.store, support.repository)
}

/// Check the repository shape and refs.
pub(crate) fn check_structure(
    path: &Utf8Path,
    commit: &str,
    store: &dyn SnapshotStore,
    repository: &dyn RepositoryQueries,
) -> Result<(), ClippySnapshotError> {
    if !store.is_present_dir(path)? {
        return Err(ClippySnapshotError::NotInstalled {
            path: path.to_string(),
        });
    }
    let evidence = repository.structure(path)?;
    accept_structure(&evidence, commit).map_err(|check| ClippySnapshotError::invalid(path, check))
}

/// Check shallow history, alternates and remotes.
fn check_self_contained(
    path: &Utf8Path,
    store: &dyn SnapshotStore,
    repository: &dyn RepositoryQueries,
) -> Result<(), ClippySnapshotError> {
    let evidence = repository.self_contained(path)?;
    let has_alternates = store.is_present(&path.join("objects/info/alternates"))?;
    accept_self_contained(&evidence, has_alternates)
        .map_err(|check| ClippySnapshotError::invalid(path, check))
}

/// Re-hash every object and check connectivity.
fn check_objects(
    path: &Utf8Path,
    repository: &dyn RepositoryQueries,
) -> Result<(), ClippySnapshotError> {
    let evidence = repository.integrity(path)?;
    accept_integrity(&evidence).map_err(|check| ClippySnapshotError::invalid(path, check))
}

/// Re-run Dylint's selection and require the recorded result.
fn check_selection(
    path: &Utf8Path,
    manifest: &ClippySnapshotManifest,
    repository: &dyn RepositoryQueries,
) -> Result<(), ClippySnapshotError> {
    let changes = toolchain_changes(repository, path, manifest.commit.as_str())?;
    // The channel is needed at the revision Dylint selects; a selection gap
    // is decided by the policy below, so there is no channel to read.
    let channel = match select_revision(&changes, &manifest.toolchain) {
        Ok(selected) => channel_at(repository, path, &selected.commit)?,
        Err(_) => None,
    };
    accept_selection(
        &changes,
        &manifest.toolchain,
        manifest.selected_revision.as_str(),
        channel.as_deref(),
    )
    .map_err(|rejection| selection_error(path, &manifest.toolchain, rejection))
}

/// Translate a pure selection rejection into the boundary error.
fn selection_error(
    path: &Utf8Path,
    toolchain: &str,
    rejection: SelectionRejection,
) -> ClippySnapshotError {
    match rejection {
        SelectionRejection::Insufficient(gap) => ClippySnapshotError::InsufficientHistory {
            path: path.to_string(),
            toolchain: toolchain.to_owned(),
            reason: gap.to_string(),
        },
        SelectionRejection::Revision { selected, expected } => ClippySnapshotError::invalid(
            path,
            format!(
                "Dylint would select {selected} for {toolchain}, but the manifest records \
                 {expected}"
            ),
        ),
        SelectionRejection::Channel {
            selected,
            channel,
            toolchain: rejected,
        } => ClippySnapshotError::invalid(
            path,
            format!(
                "the selected revision {selected} names {}, not {rejected}",
                channel.as_deref().unwrap_or("no channel")
            ),
        ),
    }
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
