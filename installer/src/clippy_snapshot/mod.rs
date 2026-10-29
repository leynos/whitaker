//! The verified Clippy history Dylint's driver build reads.
//!
//! Dylint builds its driver against Clippy source selected from Clippy's Git
//! history for the pinned nightly. Left alone it clones public Clippy for
//! that, which breaks whenever upstream history changes shape, as the
//! `master` to `main` rename did. Whitaker instead publishes the history as a
//! commit-named Git bundle and points `DYLINT_CLIPPY_REPO` at a verified local
//! clone of it.
//!
//! The producer (`whitaker-clippy-snapshot build`), the producer's own test
//! provisioning (`whitaker-clippy-snapshot install`), and the installed
//! `whitaker-installer` all go through this one module, so every path applies
//! the same identity, history, and integrity checks:
//!
//! - [`build_bundle`] clones upstream once and writes the bundle and sidecar.
//! - [`provision`] installs the pinned generation from the release or a
//!   local bundle, reusing one that already verifies.
//! - [`download_bundle`] fetches a published bundle, checksum first.
//! - [`install_bundle`] verifies a bundle and materializes it as an immutable
//!   generation at `<root>/<commit>/`.
//! - [`verify_repository`] and [`check_installed`] are the full and warm-path
//!   checks on an installed generation.
//!
//! Nothing here falls back to public Clippy. Only [`build_bundle`] may contact
//! a Git remote; [`provision`] downloads nothing but Whitaker's own release
//! asset, and every other Git command forbids all transports but `file`.

mod build;
mod download;
mod error;
mod generation;
mod git;
mod install;
mod manifest;
mod probe;
mod provision;
mod selection;
mod telemetry;
mod verify;

pub use build::{BuildRequest, BuiltBundle, build_bundle};
pub use download::download_bundle;
pub use error::ClippySnapshotError;
pub use generation::{GenerationMetadata, METADATA_FILE};
// Only the test fixture's Git shares this list outside the module.
#[cfg(any(test, feature = "test-support"))]
pub(crate) use git::REPOSITORY_SELECTION_VARIABLES;
pub use install::{BundleArtefact, install_bundle, reuse_generation};
pub use manifest::{
    ClippySnapshotManifest, SCHEMA_VERSION, SNAPSHOT_BRANCH, embedded_manifest, manifest_contents,
    parse_manifest,
};
pub use provision::{BundleSource, provision, provision_with};
pub use selection::{SelectionGap, ToolchainChange, select_revision};
pub use verify::{check_installed, verify_repository};

use camino::{Utf8Path, Utf8PathBuf};

/// Return where the generation `manifest` pins lives under `root`.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_installer::clippy_snapshot::{embedded_manifest, generation_path};
///
/// let manifest = embedded_manifest()?;
/// let path = generation_path(Utf8Path::new("/data/clippy"), manifest);
/// assert_eq!(path.parent(), Some(Utf8Path::new("/data/clippy")));
/// assert!(path.as_str().ends_with(manifest.commit.as_str()));
/// # Ok::<(), whitaker_installer::clippy_snapshot::ClippySnapshotError>(())
/// ```
#[must_use]
pub fn generation_path(root: &Utf8Path, manifest: &ClippySnapshotManifest) -> Utf8PathBuf {
    root.join(manifest.commit.as_str())
}

#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod tests;
