//! Artefact packaging for prebuilt lint library distribution.
//!
//! Creates `.tar.zst` archives containing compiled lint libraries and a
//! `manifest.json` file, following the naming and schema conventions
//! defined in ADR-001.

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
/// Returns [`PackagingError::Io`] if the file cannot be read.
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
    // sha2 always produces valid 64-char lowercase hex.
    Ok(Sha256Digest::try_from(hex).expect("sha2 produces valid 64-char lowercase hex"))
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
pub fn generate_manifest_json(manifest: &Manifest) -> Result<String, PackagingError> {
    Ok(serde_json::to_string_pretty(manifest)?)
}

/// Package compiled lint libraries into a `.tar.zst` archive.
///
/// Orchestrates: validate inputs, write manifest, create archive,
/// compute SHA-256, then rebuild the archive with the final digest
/// embedded in the manifest.
///
/// # Errors
///
/// Returns [`PackagingError::EmptyFileList`] if `params.library_files`
/// is empty, or [`PackagingError::Io`] / [`PackagingError::Serialization`]
/// on I/O or serialization failures.
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

    let file_names = collect_file_names(&params.library_files);
    let lib_entries = build_archive_entries(&params.library_files, &file_names);

    // First pass: placeholder digest so we can write the archive.
    let placeholder =
        Sha256Digest::try_from("0".repeat(64).as_str()).expect("placeholder is valid hex");
    let manifest_path = params.output_dir.join("manifest.json");
    let layout = ArchiveLayout {
        manifest_path: &manifest_path,
        lib_entries: &lib_entries,
        archive_path: &archive_path,
    };

    write_manifest_and_archive(&params, &file_names, &placeholder, &layout)?;

    // Second pass: embed the real digest.
    let real_digest = compute_sha256(&archive_path)?;
    write_manifest_and_archive(&params, &file_names, &real_digest, &layout)?;

    let _ = fs::remove_file(&manifest_path);

    let final_manifest = build_manifest(&params, file_names, real_digest);
    Ok(PackageOutput {
        archive_path,
        manifest: final_manifest,
    })
}

/// Collect filenames from library paths.
fn collect_file_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect()
}

/// Build `(source_path, archive_name)` pairs for the archive builder.
fn build_archive_entries(paths: &[PathBuf], names: &[String]) -> Vec<(PathBuf, String)> {
    paths
        .iter()
        .zip(names.iter())
        .map(|(p, n)| (p.clone(), n.clone()))
        .collect()
}

/// Paths and pre-computed entries for a single archive build pass.
struct ArchiveLayout<'a> {
    /// Where to write the manifest JSON.
    manifest_path: &'a Path,
    /// Pre-built `(source_path, archive_name)` pairs for library files.
    lib_entries: &'a [(PathBuf, String)],
    /// Destination path for the `.tar.zst` archive.
    archive_path: &'a Path,
}

/// Write the manifest JSON file and create the archive in one step.
fn write_manifest_and_archive(
    params: &PackageParams,
    file_names: &[String],
    digest: &Sha256Digest,
    layout: &ArchiveLayout<'_>,
) -> Result<(), PackagingError> {
    let manifest = build_manifest(params, file_names.to_vec(), digest.clone());
    let json = generate_manifest_json(&manifest)?;
    fs::write(layout.manifest_path, json)?;

    let mut entries = layout.lib_entries.to_vec();
    entries.push((
        layout.manifest_path.to_path_buf(),
        "manifest.json".to_owned(),
    ));
    create_archive(layout.archive_path, &entries)
}

/// Build a [`Manifest`] from packaging parameters.
fn build_manifest(
    params: &PackageParams,
    file_names: Vec<String>,
    sha256: Sha256Digest,
) -> Manifest {
    let provenance = ManifestProvenance {
        git_sha: params.git_sha.clone(),
        schema_version: SchemaVersion::current(),
        toolchain: params.toolchain.clone(),
        target: params.target.clone(),
    };
    let content = ManifestContent {
        generated_at: params.generated_at.clone(),
        files: file_names,
        sha256,
    };
    Manifest::new(provenance, content)
}

#[cfg(test)]
#[path = "packaging_tests.rs"]
mod tests;
