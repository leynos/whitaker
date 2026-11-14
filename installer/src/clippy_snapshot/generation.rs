//! The record each installed snapshot generation keeps about itself.
//!
//! A generation is a bare repository at `<root>/<commit>/`. The metadata file
//! beside its object database records what was installed and from which
//! bundle, so a later run can tell a complete generation from a directory that
//! merely has the right name. This module owns only the record's shape and the
//! rules for comparing it; reading and writing it belong to the
//! [`super::ports::MetadataStore`] adapter, which persists it through a
//! temporary file and a rename so a reader never sees half a record.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use super::{error::ClippySnapshotError, manifest::ClippySnapshotManifest};

/// The metadata file name inside a generation.
pub const METADATA_FILE: &str = "whitaker-clippy-snapshot.json";

/// The metadata schema this installer writes and accepts.
pub(crate) const METADATA_SCHEMA: u32 = 1;

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
}
