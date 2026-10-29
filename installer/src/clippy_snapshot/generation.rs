//! The record each installed snapshot generation keeps about itself.
//!
//! A generation is a bare repository at `<root>/<commit>/`. The metadata file
//! beside its object database records what was installed and from which
//! bundle, so a later run can tell a complete generation from a directory that
//! merely has the right name. It is written through a temporary file and
//! persisted with a rename, so a reader never sees half a record.

use super::error::ClippySnapshotError;
use super::manifest::ClippySnapshotManifest;
use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use std::io::Write;

/// The metadata file name inside a generation.
pub const METADATA_FILE: &str = "whitaker-clippy-snapshot.json";

/// The metadata schema this installer writes and accepts.
const METADATA_SCHEMA: u32 = 1;

/// What an installed generation holds and where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationMetadata {
    /// The metadata schema version.
    pub schema_version: u32,
    /// The pinned `main` commit.
    pub commit: String,
    /// The nightly the selection was made for.
    pub toolchain: String,
    /// The revision Dylint selects.
    pub selected_revision: String,
    /// The Dylint release that selection belongs to.
    pub dylint_version: String,
    /// The SHA-256 of the bundle the generation was cloned from.
    pub bundle_sha256: String,
}

impl GenerationMetadata {
    /// Describe a generation installed from a bundle with `bundle_sha256`.
    #[must_use]
    pub fn for_manifest(manifest: &ClippySnapshotManifest, bundle_sha256: &str) -> Self {
        Self {
            schema_version: METADATA_SCHEMA,
            commit: manifest.commit.to_string(),
            toolchain: manifest.toolchain.clone(),
            selected_revision: manifest.selected_revision.to_string(),
            dylint_version: manifest.dylint_version.clone(),
            bundle_sha256: bundle_sha256.to_owned(),
        }
    }

    /// Read the metadata of the generation at `generation`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::NotInstalled`] when the generation is
    /// absent and [`ClippySnapshotError::Invalid`] when its metadata is
    /// missing, unreadable, or from an unknown schema.
    pub fn read(generation: &Utf8Path) -> Result<Self, ClippySnapshotError> {
        if !generation.is_dir() {
            return Err(ClippySnapshotError::NotInstalled {
                path: generation.to_string(),
            });
        }
        let path = generation.join(METADATA_FILE);
        let contents = std::fs::read_to_string(&path).map_err(|error| {
            ClippySnapshotError::invalid(
                generation,
                format!("{METADATA_FILE} is unreadable: {error}"),
            )
        })?;
        let metadata: Self = serde_json::from_str(&contents).map_err(|error| {
            ClippySnapshotError::invalid(
                generation,
                format!("{METADATA_FILE} is malformed: {error}"),
            )
        })?;
        if metadata.schema_version != METADATA_SCHEMA {
            return Err(ClippySnapshotError::invalid(
                generation,
                format!(
                    "{METADATA_FILE} has schema {}, expected {METADATA_SCHEMA}",
                    metadata.schema_version
                ),
            ));
        }
        Ok(metadata)
    }

    /// Require that this metadata describes the generation `manifest` pins.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Invalid`] naming the first field that
    /// disagrees.
    pub fn check_matches(
        &self,
        generation: &Utf8Path,
        manifest: &ClippySnapshotManifest,
    ) -> Result<(), ClippySnapshotError> {
        let expected = Self::for_manifest(manifest, &self.bundle_sha256);
        let mismatch = [
            ("commit", &self.commit, &expected.commit),
            ("toolchain", &self.toolchain, &expected.toolchain),
            (
                "selected_revision",
                &self.selected_revision,
                &expected.selected_revision,
            ),
            (
                "dylint_version",
                &self.dylint_version,
                &expected.dylint_version,
            ),
        ]
        .into_iter()
        .find(|(_, actual, wanted)| actual != wanted);
        match mismatch {
            None => Ok(()),
            Some((field, actual, wanted)) => Err(ClippySnapshotError::invalid(
                generation,
                format!("{METADATA_FILE} records {field} {actual}, expected {wanted}"),
            )),
        }
    }

    /// Persist this metadata inside `generation` atomically.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the file cannot be written.
    pub fn write(&self, generation: &Utf8Path) -> Result<(), ClippySnapshotError> {
        let io_error = |error: &std::io::Error| ClippySnapshotError::io(generation, error);
        let json = serde_json::to_vec_pretty(self).map_err(|error| ClippySnapshotError::Io {
            path: generation.to_string(),
            reason: error.to_string(),
        })?;
        let mut file = tempfile::NamedTempFile::new_in(generation).map_err(|e| io_error(&e))?;
        file.write_all(&json).map_err(|e| io_error(&e))?;
        file.as_file().sync_all().map_err(|e| io_error(&e))?;
        file.persist(generation.join(METADATA_FILE))
            .map_err(|error| io_error(&error.error))?;
        Ok(())
    }
}
