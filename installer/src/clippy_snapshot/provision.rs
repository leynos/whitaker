//! One entry point for "make the pinned snapshot available under this root".
//!
//! The installer, the producer's `make clippy-snapshot`, and CI all call
//! [`provision`]. An existing generation is verified and reused; otherwise the
//! bundle comes from the Whitaker release or from an explicit local file.
//! There is no third source: when neither works, provisioning fails.

use super::download::download_bundle;
use super::error::ClippySnapshotError;
use super::install::{BundleArtefact, install_bundle, reuse_generation};
use super::manifest::ClippySnapshotManifest;
use crate::dependency_binaries::{DependencyArchiveDownloader, RepositoryArchiveDownloader};
use camino::{Utf8Path, Utf8PathBuf};

/// Where a missing generation's bundle comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleSource {
    /// The asset published on the Whitaker release.
    Release,
    /// A bundle on disk, with its `.sha256` sidecar beside it.
    Local(Utf8PathBuf),
}

/// Ensure the generation `manifest` pins is installed under `root`.
///
/// Returns the absolute generation path for `DYLINT_CLIPPY_REPO`.
///
/// # Errors
///
/// Returns any [`ClippySnapshotError`] raised while downloading, verifying,
/// or installing; none of them triggers a fallback to public Clippy.
pub fn provision(
    source: &BundleSource,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    provision_with(source, manifest, root, &RepositoryArchiveDownloader)
}

/// [`provision`] with an injectable release downloader.
///
/// # Errors
///
/// As for [`provision`].
pub fn provision_with(
    source: &BundleSource,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    downloader: &dyn DependencyArchiveDownloader,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    std::fs::create_dir_all(root).map_err(|error| ClippySnapshotError::io(root, &error))?;
    let root = std::path::absolute(root).map_err(|error| ClippySnapshotError::io(root, &error))?;
    let root = Utf8PathBuf::try_from(root).map_err(|error| ClippySnapshotError::Io {
        path: error.as_path().display().to_string(),
        reason: "the path is not valid UTF-8".to_owned(),
    })?;
    let generation = super::generation_path(&root, manifest);
    if generation.exists() {
        return reuse_generation(&generation, manifest);
    }
    match source {
        BundleSource::Local(bundle) => {
            let artefact = BundleArtefact::from_sidecar(bundle, manifest)?;
            install_bundle(&artefact, manifest, &root)
        }
        BundleSource::Release => {
            // Download beside the generations so the clone stays on one filesystem.
            let downloads = tempfile::Builder::new()
                .prefix(".download-")
                .tempdir_in(&root)
                .map_err(|error| ClippySnapshotError::io(&root, &error))?;
            let directory =
                Utf8Path::from_path(downloads.path()).ok_or_else(|| ClippySnapshotError::Io {
                    path: downloads.path().display().to_string(),
                    reason: "the path is not valid UTF-8".to_owned(),
                })?;
            let artefact = download_bundle(downloader, manifest, directory)?;
            install_bundle(&artefact, manifest, &root)
        }
    }
}
