//! One entry point for "make the pinned snapshot available under this root".
//!
//! The installer, the producer's `make clippy-snapshot`, and CI all call
//! [`provision`]. An existing generation is verified and reused; otherwise the
//! bundle comes from the Whitaker release or from an explicit local file.
//! There is no third source: when neither works, provisioning fails.
//!
//! Provisioning composes the production ports once and hands them to the
//! install and verification orchestration, so the downloader is the only
//! collaborator a test needs to replace.

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    download::download_bundle,
    error::ClippySnapshotError,
    install::{from_sidecar_with, install_with, reuse_with},
    manifest::ClippySnapshotManifest,
    ports::SnapshotSupport,
    telemetry::{Operation, ProvisionSource, record_provision, timed},
};
use crate::dependency_binaries::{DependencyArchiveDownloader, RepositoryArchiveDownloader};

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
    let support = SnapshotSupport {
        downloader,
        ..SnapshotSupport::production()
    };
    provision_support(source, manifest, root, &support)
}

/// Provision using injected ports.
fn provision_support(
    source: &BundleSource,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    support: &SnapshotSupport<'_>,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    let root = support.store.create_root(root)?;
    let generation = super::generation_path(&root, manifest);
    // An uninspectable generation is an error, not a reason to download.
    let (origin, result) = if support.store.is_present(&generation)? {
        tracing::debug!(commit = %manifest.commit, "reusing the installed Clippy snapshot");
        (
            ProvisionSource::Existing,
            timed(Operation::Reuse, || {
                reuse_with(&generation, manifest, support)
            }),
        )
    } else {
        tracing::info!(commit = %manifest.commit, "installing the pinned Clippy snapshot");
        install_from(source, manifest, &root, support)
    };
    record_provision(origin, result.is_ok());
    result
}

/// Install a missing generation from `source`, naming the source it used.
fn install_from(
    source: &BundleSource,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    support: &SnapshotSupport<'_>,
) -> (ProvisionSource, Result<Utf8PathBuf, ClippySnapshotError>) {
    match source {
        BundleSource::Local(bundle) => (
            ProvisionSource::Local,
            timed(Operation::Install, || {
                from_sidecar_with(support.store, bundle, manifest)
                    .and_then(|artefact| install_with(&artefact, manifest, root, support))
            }),
        ),
        BundleSource::Release => (
            ProvisionSource::Release,
            install_release(manifest, root, support),
        ),
    }
}

/// Download the release bundle beside the generations and install it.
fn install_release(
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    support: &SnapshotSupport<'_>,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    // Download beside the generations so the clone stays on one filesystem.
    let downloads = support.store.staging(root, ".download-")?;
    let artefact = timed(Operation::Download, || {
        download_bundle(support.downloader, manifest, downloads.path())
    })?;
    timed(Operation::Install, || {
        install_with(&artefact, manifest, root, support)
    })
}

#[cfg(test)]
#[path = "provision_tests.rs"]
mod tests;
