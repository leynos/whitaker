//! Materialize a verified bundle as an immutable snapshot generation.
//!
//! Installation never edits a repository it has accepted. A new generation is
//! cloned into a temporary sibling, verified there, and renamed into
//! `<root>/<commit>/` only once every check has passed, so a failed or
//! interrupted install leaves no directory under the generation's name. An
//! existing generation is verified and reused, or rejected; it is never
//! repaired in place.

use super::error::ClippySnapshotError;
use super::generation::GenerationMetadata;
use super::git::SnapshotGit;
use super::manifest::{ClippySnapshotManifest, SNAPSHOT_BRANCH};
use super::verify::verify_repository;
use crate::artefact::packaging::compute_sha256;
use crate::artefact::sha256_digest::Sha256Digest;
use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;

/// Serializes installers sharing one snapshot root.
const ROOT_LOCK_FILE: &str = ".whitaker-clippy-snapshot.lock";

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
        if !bundle.is_file() {
            return Err(ClippySnapshotError::BundleMissing {
                path: bundle.to_string(),
                asset: manifest.bundle_asset_name(),
            });
        }
        let sidecar = Utf8PathBuf::from(format!("{bundle}.sha256"));
        let sidecar_error = |reason: String| ClippySnapshotError::Sidecar {
            path: sidecar.to_string(),
            reason,
        };
        let contents =
            std::fs::read_to_string(&sidecar).map_err(|error| sidecar_error(error.to_string()))?;
        let token = contents
            .split_whitespace()
            .next()
            .ok_or_else(|| sidecar_error("it is empty".to_owned()))?;
        let sha256 =
            Sha256Digest::try_from(token).map_err(|error| sidecar_error(error.to_string()))?;
        Ok(Self {
            bundle: bundle.to_owned(),
            sha256,
        })
    }
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
    verify_bundle_digest(artefact)?;
    verify_bundle_heads(&artefact.bundle, manifest)?;
    std::fs::create_dir_all(root).map_err(|error| ClippySnapshotError::io(root, &error))?;
    let root = absolute(root)?;
    let _lock = lock_root(&root)?;
    let generation = root.join(manifest.commit.as_str());
    if generation.exists() {
        return reuse_generation(&generation, manifest);
    }
    let staging = tempfile::Builder::new()
        .prefix(".incoming-")
        .tempdir_in(&root)
        .map_err(|error| ClippySnapshotError::io(&root, &error))?;
    let staged = utf8(staging.path())?.join("snapshot.git");
    clone_bundle(&artefact.bundle, &staged)?;
    verify_repository(&staged, manifest)?;
    GenerationMetadata::for_manifest(manifest, artefact.sha256.as_str()).write(&staged)?;
    std::fs::rename(&staged, &generation)
        .map_err(|error| ClippySnapshotError::io(&generation, &error))?;
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
    GenerationMetadata::read(generation)?.check_matches(generation, manifest)?;
    verify_repository(generation, manifest)?;
    Ok(generation.to_owned())
}

/// Require the bundle's bytes to match the published digest.
fn verify_bundle_digest(artefact: &BundleArtefact) -> Result<(), ClippySnapshotError> {
    let actual =
        compute_sha256(artefact.bundle.as_std_path()).map_err(|error| ClippySnapshotError::Io {
            path: artefact.bundle.to_string(),
            reason: error.to_string(),
        })?;
    if actual == artefact.sha256 {
        return Ok(());
    }
    Err(ClippySnapshotError::ChecksumMismatch {
        path: artefact.bundle.to_string(),
        expected: artefact.sha256.to_string(),
        actual: actual.to_string(),
    })
}

/// Run `git bundle verify` and require exactly the pinned heads.
///
/// `git bundle verify` insists on a repository even for a bundle without
/// prerequisites, so it runs inside an empty scratch repository.
fn verify_bundle_heads(
    bundle: &Utf8Path,
    manifest: &ClippySnapshotManifest,
) -> Result<(), ClippySnapshotError> {
    let git = SnapshotGit::local();
    let scratch = tempfile::tempdir().map_err(|error| ClippySnapshotError::io(bundle, &error))?;
    let scratch = utf8(scratch.path())?.to_owned();
    git.stdout(
        "init",
        None,
        &["init", "--quiet", "--bare", scratch.as_str()],
    )?;
    let verify = git.output(
        "bundle verify",
        Some(&scratch),
        &["bundle", "verify", bundle.as_str()],
    )?;
    if !verify.status.success() {
        let stderr = String::from_utf8_lossy(&verify.stderr);
        return Err(ClippySnapshotError::invalid(
            bundle,
            format!("git bundle verify failed: {}", stderr.trim()),
        ));
    }
    let heads = git.stdout(
        "bundle list-heads",
        Some(&scratch),
        &["bundle", "list-heads", bundle.as_str()],
    )?;
    let commit = manifest.commit.as_str();
    let main = format!("refs/heads/{SNAPSHOT_BRANCH}");
    let unexpected: Vec<&str> = heads
        .lines()
        .filter(|line| *line != format!("{commit} {main}") && *line != format!("{commit} HEAD"))
        .collect();
    if !unexpected.is_empty() {
        return Err(ClippySnapshotError::invalid(
            bundle,
            format!(
                "the bundle must carry only {main} and HEAD at {commit}, but lists: {}",
                unexpected.join("; ")
            ),
        ));
    }
    if !heads.lines().any(|line| line == format!("{commit} {main}")) {
        return Err(ClippySnapshotError::invalid(
            bundle,
            format!("the bundle does not carry {main} at the pinned commit {commit}"),
        ));
    }
    Ok(())
}

/// Clone `bundle` as a bare repository with no remote.
fn clone_bundle(bundle: &Utf8Path, destination: &Utf8Path) -> Result<(), ClippySnapshotError> {
    let git = SnapshotGit::local();
    // `git bundle verify` reads only the header, so a truncated or damaged
    // pack first shows up here.
    git.stdout(
        "clone",
        None,
        &[
            "clone",
            "--quiet",
            "--bare",
            bundle.as_str(),
            destination.as_str(),
        ],
    )
    .map_err(|error| {
        ClippySnapshotError::invalid(bundle, format!("the bundle could not be cloned: {error}"))
    })?;
    git.stdout(
        "remote remove",
        Some(destination),
        &["remote", "remove", "origin"],
    )?;
    Ok(())
}

/// Hold an exclusive lock on the snapshot root until the guard drops.
fn lock_root(root: &Utf8Path) -> Result<std::fs::File, ClippySnapshotError> {
    let path = root.join(ROOT_LOCK_FILE);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| ClippySnapshotError::io(&path, &error))?;
    file.lock_exclusive()
        .map_err(|error| ClippySnapshotError::io(&path, &error))?;
    Ok(file)
}

/// Resolve `path` against the current directory, as `DYLINT_CLIPPY_REPO` needs.
fn absolute(path: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
    let resolved =
        std::path::absolute(path).map_err(|error| ClippySnapshotError::io(path, &error))?;
    Ok(utf8(&resolved)?.to_owned())
}

/// Require a UTF-8 path, which the environment variables Dylint reads need.
fn utf8(path: &std::path::Path) -> Result<&Utf8Path, ClippySnapshotError> {
    Utf8Path::from_path(path).ok_or_else(|| ClippySnapshotError::Io {
        path: path.display().to_string(),
        reason: "the path is not valid UTF-8".to_owned(),
    })
}
