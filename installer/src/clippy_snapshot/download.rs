//! Fetch the published snapshot bundle from the Whitaker release.
//!
//! The bundle travels through the same downloader as the dependency
//! binaries: the bytes are written, the `.sha256` sidecar is fetched, and the
//! file is deleted unless the two agree. Only then is the bundle handed to
//! [`super::install_bundle`], which checks the digest again against the bytes
//! it actually clones.

use camino::Utf8Path;

use super::{
    error::ClippySnapshotError,
    install::BundleArtefact,
    manifest::ClippySnapshotManifest,
    ports::SnapshotStore,
    store::ProductionStore,
};
use crate::dependency_binaries::DependencyArchiveDownloader;

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
    download_bundle_with(&ProductionStore, downloader, manifest, directory)
}

/// Download the bundle, reading its digest through `store`.
///
/// Provisioning passes the store it already holds, so a scripted store can
/// stand in for the filesystem and the digest read is observable.
///
/// # Errors
///
/// As for [`download_bundle`], plus [`ClippySnapshotError::Io`] when the
/// downloaded bytes cannot be hashed.
pub(crate) fn download_bundle_with(
    store: &dyn SnapshotStore,
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
    let sha256 = store.digest(&bundle)?;
    Ok(BundleArtefact { bundle, sha256 })
}
