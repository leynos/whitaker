//! Full verification of the installed snapshot against its own record.
//!
//! `run` calls this wherever Dylint may read the snapshot to build a driver:
//! before every cold build, and after a warm run whose child rebuilt one.

use super::error::BootstrapError;
use crate::clippy_snapshot::{ClippySnapshotManifest, GenerationMetadata, verify_repository};
use crate::git::CommitSha;
use camino::Utf8Path;

/// Fully verify the snapshot against its own installation record.
pub(super) fn verify_snapshot(snapshot: &Utf8Path) -> Result<(), BootstrapError> {
    let generation = GenerationMetadata::read(snapshot)?;
    let invalid = |field: &str, error: &dyn std::fmt::Display| {
        crate::clippy_snapshot::ClippySnapshotError::invalid(
            snapshot,
            format!("its metadata {field} is not a commit: {error}"),
        )
    };
    let manifest = ClippySnapshotManifest {
        upstream: String::new(),
        commit: CommitSha::try_from(generation.commit.as_str())
            .map_err(|e| invalid("commit", &e))?,
        toolchain: generation.toolchain.clone(),
        selected_revision: CommitSha::try_from(generation.selected_revision.as_str())
            .map_err(|e| invalid("selected_revision", &e))?,
        dylint_version: generation.dylint_version.clone(),
    };
    verify_repository(snapshot, &manifest)?;
    Ok(())
}
