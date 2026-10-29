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
use super::probe::is_present;
use super::telemetry::{Operation, ProvisionSource, record_provision, timed};
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
#[tracing::instrument(name = "clippy_snapshot.provision", skip_all, fields(commit = %manifest.commit))]
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
    // An uninspectable generation is an error, not a reason to download.
    let (origin, result) = if is_present(&generation)? {
        tracing::debug!(commit = %manifest.commit, "reusing the installed Clippy snapshot");
        (
            ProvisionSource::Existing,
            timed(Operation::Reuse, || reuse_generation(&generation, manifest)),
        )
    } else {
        tracing::info!(commit = %manifest.commit, "installing the pinned Clippy snapshot");
        install_from(source, manifest, &root, downloader)
    };
    record_provision(origin, result.is_ok());
    result
}

/// Install a missing generation from `source`, naming the source it used.
fn install_from(
    source: &BundleSource,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    downloader: &dyn DependencyArchiveDownloader,
) -> (ProvisionSource, Result<Utf8PathBuf, ClippySnapshotError>) {
    match source {
        BundleSource::Local(bundle) => (
            ProvisionSource::Local,
            timed(Operation::Install, || {
                BundleArtefact::from_sidecar(bundle, manifest)
                    .and_then(|artefact| install_bundle(&artefact, manifest, root))
            }),
        ),
        BundleSource::Release => (
            ProvisionSource::Release,
            install_release(manifest, root, downloader),
        ),
    }
}

/// Download the release bundle beside the generations and install it.
fn install_release(
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    downloader: &dyn DependencyArchiveDownloader,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    // Download beside the generations so the clone stays on one filesystem.
    let downloads = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(root)
        .map_err(|error| ClippySnapshotError::io(root, &error))?;
    let directory =
        Utf8Path::from_path(downloads.path()).ok_or_else(|| ClippySnapshotError::Io {
            path: downloads.path().display().to_string(),
            reason: "the path is not valid UTF-8".to_owned(),
        })?;
    let artefact = timed(Operation::Download, || {
        download_bundle(downloader, manifest, directory)
    })?;
    timed(Operation::Install, || {
        install_bundle(&artefact, manifest, root)
    })
}

#[cfg(test)]
#[path = "provision_tests.rs"]
mod tests;
