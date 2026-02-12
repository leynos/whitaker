//! Artefact packaging for prebuilt lint library distribution.
//!
//! Creates `.tar.zst` archives containing compiled lint libraries and a
//! `manifest.json` file, following the naming and schema conventions
//! defined in ADR-001.
//!
//! # Preconditions
//!
//! - All library file paths in [`PackageParams::library_files`] must
//!   exist on disk and have a filename component.
//! - The `output_dir` must exist and be writable.
//!
//! # Outputs and side effects
//!
//! [`package_artefact`] writes a `.tar.zst` archive to the output
//! directory and returns a [`PackageOutput`] containing the archive
//! path and an in-memory [`Manifest`].  A temporary `manifest.json`
//! file is created during packaging and removed afterwards.
//!
//! # Two-pass SHA-256 algorithm
//!
//! The archive digest is computed over the first-pass archive (which
//! contains a placeholder digest in its embedded manifest).  The
//! second-pass archive embeds this real digest.  Consumers verify
//! integrity by computing the SHA-256 of the downloaded archive and
//! comparing against the `sha256` field returned by a separate
//! manifest query (e.g. from the release API), not by re-extracting
//! the manifest from the archive they just hashed.

use super::git_sha::GitSha;
use super::manifest::{GeneratedAt, Manifest, ManifestContent, ManifestProvenance};
use super::naming::ArtefactName;
use super::packaging_error::PackagingError;
use super::schema_version::SchemaVersion;
use super::sha256_digest::Sha256Digest;
use super::target::TargetTriple;
use super::toolchain_channel::ToolchainChannel;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Input parameters for the [`package_artefact`] function.
///
/// Groups all required inputs so the function signature stays within
/// Clippy's parameter limit.
#[derive(Debug)]
pub struct PackageParams {
    /// The git commit SHA identifying this build.
    pub git_sha: GitSha,
    /// The Rust toolchain channel used for the build.
    pub toolchain: ToolchainChannel,
    /// The target triple the libraries were compiled for.
    pub target: TargetTriple,
    /// Paths to the compiled library files to include in the archive.
    pub library_files: Vec<PathBuf>,
    /// Directory where the output archive will be written.
    pub output_dir: PathBuf,
    /// ISO 8601 timestamp for the build.
    pub generated_at: GeneratedAt,
}

/// Output produced by [`package_artefact`].
#[derive(Debug)]
pub struct PackageOutput {
    /// Path to the created `.tar.zst` archive.
    pub archive_path: PathBuf,
    /// The manifest describing the archive contents.
    pub manifest: Manifest,
}

/// Compute the SHA-256 digest of a file.
///
/// Reads the file at `path` in chunks and returns the lowercase hex
/// digest as a validated [`Sha256Digest`].
///
/// # Errors
///
/// Returns [`PackagingError::Io`] if the file cannot be read, or
/// [`PackagingError::InvalidDigest`] if the hex conversion fails.
pub fn compute_sha256(path: &Path) -> Result<Sha256Digest, PackagingError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let hex = format!("{:x}", hasher.finalize());
    Ok(Sha256Digest::try_from(hex)?)
}

/// Create a `.tar.zst` archive at `output_path`.
///
/// Each entry in `files` is a `(source_path, archive_name)` pair. The
/// `archive_name` determines the filename inside the tar archive,
/// allowing files to be renamed during packaging.
///
/// # Errors
///
/// Returns [`PackagingError::Io`] if any source file cannot be read or
/// the output file cannot be written.
pub fn create_archive(
    output_path: &Path,
    files: &[(PathBuf, String)],
) -> Result<(), PackagingError> {
    let output_file = fs::File::create(output_path)?;
    let zstd_encoder = zstd::Encoder::new(output_file, 0)?.auto_finish();
    let mut archive = tar::Builder::new(zstd_encoder);

    for (source_path, archive_name) in files {
        archive.append_path_with_name(source_path, archive_name)?;
    }

    archive.finish()?;
    Ok(())
}

/// Serialize a [`Manifest`] to pretty-printed JSON.
///
/// The output matches the ADR-001 manifest schema with all seven
/// required top-level keys.
///
/// # Errors
///
/// Returns [`PackagingError::Serialization`] if serialization fails.
///
/// # Examples
///
/// ```
/// whitaker_installer::_manifest_doc_setup!(manifest);
/// use whitaker_installer::artefact::packaging::generate_manifest_json;
///
/// let json = generate_manifest_json(&manifest).expect("serialization");
/// let parsed: serde_json::Value =
///     serde_json::from_str(&json).expect("valid JSON");
/// let obj = parsed.as_object().expect("top-level object");
/// assert!(obj.contains_key("git_sha"));
/// assert!(obj.contains_key("sha256"));
/// ```
pub fn generate_manifest_json(manifest: &Manifest) -> Result<String, PackagingError> {
    Ok(serde_json::to_string_pretty(manifest)?)
}

/// Package compiled lint libraries into a `.tar.zst` archive.
///
/// Uses a two-pass algorithm: the first pass creates an archive with a
/// placeholder SHA-256 digest in the manifest, then the real digest of
/// that archive is computed and a second pass rebuilds the archive with
/// the correct digest embedded.
///
/// # Errors
///
/// Returns [`PackagingError::EmptyFileList`] if `params.library_files`
/// is empty, [`PackagingError::InvalidLibraryPath`] if any path lacks
/// a filename, or [`PackagingError::Io`] /
/// [`PackagingError::Serialization`] on I/O or serialization failures.
pub fn package_artefact(params: PackageParams) -> Result<PackageOutput, PackagingError> {
    if params.library_files.is_empty() {
        return Err(PackagingError::EmptyFileList);
    }

    let artefact_name = ArtefactName::new(
        params.git_sha.clone(),
        params.toolchain.clone(),
        params.target.clone(),
    );
    let archive_path = params.output_dir.join(artefact_name.filename());
    let manifest_path = params.output_dir.join("manifest.json");

    let file_names = collect_file_names(&params.library_files)?;
    let lib_entries: Vec<(PathBuf, String)> = params
        .library_files
        .iter()
        .zip(&file_names)
        .map(|(p, n)| (p.clone(), n.clone()))
        .collect();

    // Helper closure that writes a manifest with the given digest and
    // creates the archive.  Captures the local paths so we stay within
    // Clippy's parameter limit.
    let write_pass = |digest: &Sha256Digest| -> Result<(), PackagingError> {
        let manifest = build_manifest(&params, &file_names, digest);
        let json = generate_manifest_json(&manifest)?;
        fs::write(&manifest_path, json)?;

        let mut entries = lib_entries.clone();
        entries.push((manifest_path.clone(), "manifest.json".to_owned()));
        create_archive(&archive_path, &entries)
    };

    // First pass: placeholder digest.
    let placeholder = Sha256Digest::try_from("0".repeat(64).as_str())?;
    write_pass(&placeholder)?;

    // Second pass: embed the real digest.
    let real_digest = compute_sha256(&archive_path)?;
    write_pass(&real_digest)?;

    let _ = fs::remove_file(&manifest_path);

    let final_manifest = build_manifest(&params, &file_names, &real_digest);
    Ok(PackageOutput {
        archive_path,
        manifest: final_manifest,
    })
}

/// Extract basenames from library paths, rejecting any without a
/// filename component.
fn collect_file_names(paths: &[PathBuf]) -> Result<Vec<String>, PackagingError> {
    paths
        .iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| PackagingError::InvalidLibraryPath(p.clone()))
        })
        .collect()
}

/// Build a [`Manifest`] from packaging parameters.
fn build_manifest(
    params: &PackageParams,
    file_names: &[String],
    sha256: &Sha256Digest,
) -> Manifest {
    let provenance = ManifestProvenance {
        git_sha: params.git_sha.clone(),
        schema_version: SchemaVersion::current(),
        toolchain: params.toolchain.clone(),
        target: params.target.clone(),
    };
    let content = ManifestContent {
        generated_at: params.generated_at.clone(),
        files: file_names.to_vec(),
        sha256: sha256.clone(),
    };
    Manifest::new(provenance, content)
}

#[cfg(test)]
#[path = "packaging_tests.rs"]
mod tests;
