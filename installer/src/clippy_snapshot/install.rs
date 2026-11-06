//! Materialize a verified bundle as an immutable snapshot generation.
//!
//! Installation never edits a repository it has accepted. A new generation is
//! cloned into a temporary sibling, verified there, and renamed into
//! `<root>/<commit>/` only once every check has passed, so a failed or
//! interrupted install leaves no directory under the generation's name. An
//! existing generation is verified and reused, or rejected; it is never
//! repaired in place.
//!
//! The orchestration observes through [`SnapshotStore`], [`BundleOperations`]
//! and [`MetadataStore`]; the accept/reject rules are pure functions in
//! [`super::policy`].

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    error::ClippySnapshotError,
    generation::GenerationMetadata,
    manifest::ClippySnapshotManifest,
    policy::{PinnedCommit, accept_bundle_heads, digests_match},
    ports::{BundleOperations, SnapshotStore, SnapshotSupport},
    store::ProductionStore,
    verify::verify_with,
};
use crate::artefact::sha256_digest::Sha256Digest;

/// A bundle on disk and the digest its publisher recorded for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleArtefact {
    /// The bundle file.
    pub bundle: Utf8PathBuf,
    /// The published SHA-256 of the bundle's bytes.
    pub sha256: Sha256Digest,
}

impl BundleArtefact {
    /// Pair `bundle` with the digest in its `<bundle>.sha256` sidecar.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::BundleMissing`] when the bundle is absent
    /// and [`ClippySnapshotError::Sidecar`] when the sidecar is absent or does
    /// not start with a 64-digit lowercase hexadecimal digest.
    pub fn from_sidecar(
        bundle: &Utf8Path,
        manifest: &ClippySnapshotManifest,
    ) -> Result<Self, ClippySnapshotError> {
        from_sidecar_with(&ProductionStore, bundle, manifest)
    }
}

/// Pair a bundle with its sidecar using an injected store.
pub(crate) fn from_sidecar_with(
    store: &dyn SnapshotStore,
    bundle: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<BundleArtefact, ClippySnapshotError> {
    if !store.is_present_file(bundle)? {
        return Err(ClippySnapshotError::BundleMissing {
            path: bundle.to_string(),
            asset: manifest.bundle_asset_name(),
        });
    }
    // Git later runs from other directories, so a caller's relative path
    // must be fixed against the caller's directory now.
    let bundle = store.absolute(bundle)?;
    let sidecar = Utf8PathBuf::from(format!("{bundle}.sha256"));
    let sidecar_error = |reason: String| ClippySnapshotError::Sidecar {
        path: sidecar.to_string(),
        reason,
    };
    // Read through the store port: the orchestration must not reach for the
    // filesystem itself, and a partly scripted store would hide the read.
    // Only the reason travels: `Io`'s display already names the path, its
    // remedy, and the no-fallback rule, and `Sidecar` repeats that advice.
    let contents = store
        .read_to_string(&sidecar)
        .map_err(|error| match error {
            ClippySnapshotError::Io { reason, .. } => sidecar_error(reason),
            other => sidecar_error(other.to_string()),
        })?;
    let token = contents
        .split_whitespace()
        .next()
        .ok_or_else(|| sidecar_error("it is empty".to_owned()))?;
    let sha256 = Sha256Digest::try_from(token).map_err(|error| sidecar_error(error.to_string()))?;
    Ok(BundleArtefact { bundle, sha256 })
}

/// Install `artefact` as the generation `manifest` pins under `root`.
///
/// Returns the absolute generation path, suitable for `DYLINT_CLIPPY_REPO`.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::ChecksumMismatch`] for corrupt bytes,
/// [`ClippySnapshotError::Invalid`] when the bundle or the resulting
/// repository fails a check, and [`ClippySnapshotError::Io`] or
/// [`ClippySnapshotError::Git`] for filesystem and subprocess failures.
pub fn install_bundle(
    artefact: &BundleArtefact,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    install_with(artefact, manifest, root, &SnapshotSupport::production())
}

/// Install a bundle using injected ports.
pub(crate) fn install_with(
    artefact: &BundleArtefact,
    manifest: &ClippySnapshotManifest,
    root: &Utf8Path,
    support: &SnapshotSupport<'_>,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    verify_bundle_digest(artefact, support.store)?;
    // `git bundle verify` runs inside a scratch repository and the clone in
    // the snapshot root, so a relative bundle path would resolve wrongly.
    let bundle = support.store.absolute(&artefact.bundle)?;
    verify_bundle_heads(&bundle, manifest, support.bundles)?;
    let root = support.store.create_root(root)?;
    let _lock = support.store.lock(&root)?;
    let generation = root.join(manifest.commit.as_str());
    if support.store.is_present(&generation)? {
        return reuse_with(&generation, manifest, support);
    }
    let staging = support.store.staging(&root, ".incoming-")?;
    let staged = staging.path().join("snapshot.git");
    support.bundles.stage_clone(&bundle, &staged)?;
    verify_with(&staged, manifest, support)?;
    let metadata = GenerationMetadata::for_manifest(manifest, artefact.sha256.as_str());
    support.metadata.write(&staged, &metadata)?;
    support.store.publish(&staged, &generation)?;
    Ok(generation)
}

/// Verify and reuse an existing generation without modifying it.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Invalid`] when the generation's metadata or
/// repository fails verification.
pub fn reuse_generation(
    generation: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    reuse_with(generation, manifest, &SnapshotSupport::production())
}

/// Reuse a generation using injected ports.
pub(crate) fn reuse_with(
    generation: &Utf8Path,
    manifest: &ClippySnapshotManifest,
    support: &SnapshotSupport<'_>,
) -> Result<Utf8PathBuf, ClippySnapshotError> {
    let metadata = support.metadata.read(generation)?;
    metadata.check_matches(generation, manifest)?;
    verify_with(generation, manifest, support)?;
    Ok(generation.to_owned())
}

/// Require the bundle's bytes to match the published digest.
fn verify_bundle_digest(
    artefact: &BundleArtefact,
    store: &dyn SnapshotStore,
) -> Result<(), ClippySnapshotError> {
    let actual = store.digest(&artefact.bundle)?;
    if digests_match(artefact.sha256.as_str(), actual.as_str()) {
        return Ok(());
    }
    Err(ClippySnapshotError::ChecksumMismatch {
        path: artefact.bundle.to_string(),
        expected: artefact.sha256.to_string(),
        actual: actual.to_string(),
    })
}

/// Run `git bundle verify` and require exactly the pinned heads.
fn verify_bundle_heads(
    bundle: &Utf8Path,
    manifest: &ClippySnapshotManifest,
    bundles: &dyn BundleOperations,
) -> Result<(), ClippySnapshotError> {
    bundles.verify_header(bundle)?;
    let heads = bundles.list_heads(bundle)?;
    accept_bundle_heads(&heads, PinnedCommit::new(manifest.commit.as_str()))
        .map_err(|check| ClippySnapshotError::invalid(bundle, check))
}
