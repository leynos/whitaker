//! Packaging helpers for repository-hosted dependency binaries.
//!
//! This mirrors the installer archive layout so release workflows can package
//! `cargo-dylint` and `dylint-link` with deterministic names and inner
//! directories for each supported target.

use crate::dependency_binaries::{DependencyBinary, binary_filename, provenance_filename};
pub use crate::installer_packaging::{ArchiveFormat, TargetTriple};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Parameters for packaging a dependency binary archive.
#[derive(Debug)]
pub struct DependencyPackageParams {
    /// The dependency metadata from `dependency-binaries.toml`.
    pub dependency: DependencyBinary,
    /// The target triple the executable was built for.
    pub target: TargetTriple,
    /// Path to the built executable.
    pub binary_path: PathBuf,
    /// Output directory where the archive should be written.
    pub output_dir: PathBuf,
}

/// Output produced by [`package_dependency_binary`].
#[derive(Debug)]
pub struct DependencyPackageOutput {
    /// Absolute path to the archive.
    pub archive_path: PathBuf,
    /// Archive filename.
    pub archive_name: String,
}

/// Errors returned during dependency-binary packaging.
#[derive(Debug, Error)]
pub enum DependencyPackagingError {
    /// I/O error while reading or writing archive contents.
    #[error("I/O error during packaging: {0}")]
    Io(#[from] std::io::Error),

    /// The requested binary does not exist.
    #[error("binary file not found: {0}")]
    BinaryNotFound(PathBuf),

    /// ZIP archive creation failed.
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Compute the top-level directory name embedded in a dependency archive.
#[must_use]
pub fn inner_dir_name(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    format!(
        "{}-{}-v{}",
        dependency.package(),
        target.as_str(),
        dependency.version()
    )
}

/// Determine the archive format for the target.
#[must_use]
pub fn archive_format(target: &TargetTriple) -> ArchiveFormat {
    if target.is_windows() {
        ArchiveFormat::Zip
    } else {
        ArchiveFormat::Tgz
    }
}

/// Package a dependency executable into the deterministic repository archive.
pub fn package_dependency_binary(
    params: DependencyPackageParams,
) -> Result<DependencyPackageOutput, DependencyPackagingError> {
    if !params.binary_path.is_file() {
        return Err(DependencyPackagingError::BinaryNotFound(
            params.binary_path.clone(),
        ));
    }

    fs::create_dir_all(&params.output_dir)?;
    let archive_name =
        crate::dependency_binaries::archive_filename(&params.dependency, &params.target);
    let archive_path = params.output_dir.join(&archive_name);
    let inner_dir = inner_dir_name(&params.dependency, &params.target);
    let binary_name = binary_filename(&params.dependency, &params.target);

    match archive_format(&params.target) {
        ArchiveFormat::Tgz => {
            create_tgz_archive(&archive_path, &inner_dir, &params.binary_path, &binary_name)?;
        }
        ArchiveFormat::Zip => {
            create_zip_archive(&archive_path, &inner_dir, &params.binary_path, &binary_name)?;
        }
    }

    let archive_path = archive_path.canonicalize()?;

    Ok(DependencyPackageOutput {
        archive_path,
        archive_name,
    })
}

/// Render the shared dependency-binary provenance and licence document.
#[must_use]
pub fn render_provenance_markdown(dependencies: &[DependencyBinary]) -> String {
    let mut output = String::from("# Dependency binary licences and provenance\n\n");
    output.push_str(
        "Whitaker publishes the following third-party dependency binaries from repository releases.\n\n",
    );
    for dependency in dependencies {
        output.push_str(&format!("## {}\n\n", dependency.package()));
        output.push_str(&format!("- Binary: `{}`\n", dependency.binary()));
        output.push_str(&format!("- Version: `{}`\n", dependency.version()));
        output.push_str(&format!("- Licence: `{}`\n", dependency.license()));
        output.push_str(&format!("- Repository: {}\n\n", dependency.repository()));
    }
    output
}

/// Write the shared provenance document to `output_dir`.
pub fn write_provenance_markdown(
    output_dir: &Path,
    dependencies: &[DependencyBinary],
) -> Result<PathBuf, DependencyPackagingError> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join(provenance_filename());
    fs::write(&path, render_provenance_markdown(dependencies))?;
    Ok(path)
}

/// Create a deterministic `.tgz` archive containing one packaged executable.
fn create_tgz_archive(
    output_path: &Path,
    inner_dir: &str,
    binary_path: &Path,
    binary_name: &str,
) -> Result<(), DependencyPackagingError> {
    let output_file = fs::File::create(output_path)?;
    let gz_encoder = flate2::write::GzEncoder::new(output_file, flate2::Compression::default());
    let mut archive = tar::Builder::new(gz_encoder);
    archive.mode(tar::HeaderMode::Deterministic);
    archive.append_path_with_name(binary_path, format!("{inner_dir}/{binary_name}"))?;
    let gz_encoder = archive.into_inner()?;
    gz_encoder.finish()?;
    Ok(())
}

/// Create a deterministic `.zip` archive containing one packaged executable.
fn create_zip_archive(
    output_path: &Path,
    inner_dir: &str,
    binary_path: &Path,
    binary_name: &str,
) -> Result<(), DependencyPackagingError> {
    let output_file = fs::File::create(output_path)?;
    let mut zip_writer = zip::ZipWriter::new(output_file);
    let options = zip::write::SimpleFileOptions::default()
        .last_modified_time(zip::DateTime::default())
        .compression_method(zip::CompressionMethod::Deflated);
    zip_writer.start_file(format!("{inner_dir}/{binary_name}"), options)?;
    let mut binary_file = fs::File::open(binary_path)?;
    let mut buffer = [0u8; 8_192];
    loop {
        let bytes_read = binary_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        zip_writer.write_all(&buffer[..bytes_read])?;
    }
    zip_writer.finish()?;
    Ok(())
}
