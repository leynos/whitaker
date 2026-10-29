//! Fetch the published snapshot bundle from the Whitaker release.
//!
//! The bundle travels through the same downloader as the dependency
//! binaries: the bytes are written, the `.sha256` sidecar is fetched, and the
//! file is deleted unless the two agree. Only then is the bundle handed to
//! [`super::install_bundle`], which checks the digest again against the bytes
//! it actually clones.

use super::error::ClippySnapshotError;
use super::install::BundleArtefact;
use super::manifest::ClippySnapshotManifest;
use crate::artefact::packaging::compute_sha256;
use crate::dependency_binaries::DependencyArchiveDownloader;
use camino::Utf8Path;

/// Download the bundle `manifest` pins into `directory`.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::AssetNotFound`] when the release does not
/// publish the bundle, and [`ClippySnapshotError::Download`] for any other
/// fetch or checksum failure.
pub fn download_bundle(
    downloader: &dyn DependencyArchiveDownloader,
    manifest: &ClippySnapshotManifest,
    directory: &Utf8Path,
) -> Result<BundleArtefact, ClippySnapshotError> {
    let asset = manifest.bundle_asset_name();
    let bundle = directory.join(&asset);
    downloader
        .download(&asset, bundle.as_std_path())
        .map_err(|error| match error {
            crate::dependency_binaries::DependencyBinaryInstallError::NotFound { url } => {
                ClippySnapshotError::AssetNotFound { url }
            }
            other => ClippySnapshotError::Download {
                asset: asset.clone(),
                reason: other.to_string(),
            },
        })?;
    // The downloader has already matched these bytes against the release
    // sidecar; recording their digest lets installation re-check the same
    // bytes rather than trusting that nothing changed in between.
    let sha256 = compute_sha256(bundle.as_std_path()).map_err(|error| ClippySnapshotError::Io {
        path: bundle.to_string(),
        reason: error.to_string(),
    })?;
    Ok(BundleArtefact { bundle, sha256 })
}
