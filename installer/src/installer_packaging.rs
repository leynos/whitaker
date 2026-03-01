//! Installer binary archive packaging for release distribution.
//!
//! Creates `.tgz` (gzip-compressed tar) or `.zip` archives containing the
//! `whitaker-installer` binary, following the naming and layout conventions
//! defined in the binstall metadata (§ Installer release artefacts).
//!
//! # Archive layout
//!
//! Each archive contains a single top-level directory named
//! `whitaker-installer-<target>-v<version>/` with the binary inside:
//!
//! ```text
//! whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1/
//!   whitaker-installer
//! ```
//!
//! Windows archives use `.zip` format and the `.exe` suffix.

use crate::binstall_metadata::{DEFAULT_PKG_FMT, WINDOWS_OVERRIDE_TARGET, WINDOWS_PKG_FMT};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// The crate name used in archive and directory names.
const CRATE_NAME: &str = "whitaker-installer";

/// Supported archive formats for installer packaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// Gzip-compressed tar archive (`.tgz`).
    Tgz,
    /// ZIP archive (`.zip`).
    Zip,
}

/// Parameters for packaging the installer binary into a release archive.
#[derive(Debug)]
pub struct InstallerPackageParams {
    /// The crate version (e.g. `"0.2.1"`).
    pub version: String,
    /// The target triple (e.g. `"x86_64-unknown-linux-gnu"`).
    pub target: String,
    /// Path to the compiled installer binary.
    pub binary_path: PathBuf,
    /// Directory where the output archive will be written.
    pub output_dir: PathBuf,
}

/// Output produced by [`package_installer`].
#[derive(Debug)]
pub struct InstallerPackageOutput {
    /// Path to the created archive file.
    pub archive_path: PathBuf,
    /// The archive filename (e.g.
    /// `"whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1.tgz"`).
    pub archive_name: String,
}

/// Errors that can occur during installer packaging.
#[derive(Debug, Error)]
pub enum InstallerPackagingError {
    /// An I/O error during archive creation.
    #[error("I/O error during packaging: {0}")]
    Io(#[from] std::io::Error),
    /// The binary file to be packaged was not found.
    #[error("binary file not found: {0}")]
    BinaryNotFound(PathBuf),
    /// A ZIP archive error.
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Compute the archive filename for a given version and target.
///
/// Returns a filename matching the binstall `pkg-url` template:
/// `whitaker-installer-<target>-v<version>.tgz` (or `.zip` for Windows).
///
/// # Examples
///
/// ```
/// use whitaker_installer::installer_packaging::archive_filename;
///
/// let name = archive_filename("0.2.1", "x86_64-unknown-linux-gnu");
/// assert_eq!(name, "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1.tgz");
/// ```
#[must_use]
pub fn archive_filename(version: &str, target: &str) -> String {
    let ext = if target == WINDOWS_OVERRIDE_TARGET {
        WINDOWS_PKG_FMT
    } else {
        DEFAULT_PKG_FMT
    };
    format!("{CRATE_NAME}-{target}-v{version}.{ext}")
}

/// Compute the inner directory name for a given version and target.
///
/// Returns the top-level directory name within the archive:
/// `whitaker-installer-<target>-v<version>`.
///
/// # Examples
///
/// ```
/// use whitaker_installer::installer_packaging::inner_dir_name;
///
/// let dir = inner_dir_name("0.2.1", "x86_64-unknown-linux-gnu");
/// assert_eq!(dir, "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1");
/// ```
#[must_use]
pub fn inner_dir_name(version: &str, target: &str) -> String {
    format!("{CRATE_NAME}-{target}-v{version}")
}

/// Compute the binary filename for a given target.
///
/// Returns `whitaker-installer.exe` for Windows targets and
/// `whitaker-installer` for all others.
///
/// # Examples
///
/// ```
/// use whitaker_installer::installer_packaging::binary_filename;
///
/// assert_eq!(binary_filename("x86_64-unknown-linux-gnu"), "whitaker-installer");
/// assert_eq!(binary_filename("x86_64-pc-windows-msvc"), "whitaker-installer.exe");
/// ```
#[must_use]
pub fn binary_filename(target: &str) -> String {
    if target == WINDOWS_OVERRIDE_TARGET {
        format!("{CRATE_NAME}.exe")
    } else {
        CRATE_NAME.to_owned()
    }
}

/// Determine the archive format for a given target.
///
/// Returns [`ArchiveFormat::Zip`] for `x86_64-pc-windows-msvc` and
/// [`ArchiveFormat::Tgz`] for all other targets.
///
/// # Examples
///
/// ```
/// use whitaker_installer::installer_packaging::{archive_format, ArchiveFormat};
///
/// assert_eq!(archive_format("x86_64-unknown-linux-gnu"), ArchiveFormat::Tgz);
/// assert_eq!(archive_format("x86_64-pc-windows-msvc"), ArchiveFormat::Zip);
/// ```
#[must_use]
pub fn archive_format(target: &str) -> ArchiveFormat {
    if target == WINDOWS_OVERRIDE_TARGET {
        ArchiveFormat::Zip
    } else {
        ArchiveFormat::Tgz
    }
}

/// Package the installer binary into the appropriate archive format.
///
/// Creates a `.tgz` or `.zip` archive (depending on the target) in the
/// output directory, with the binary nested inside a top-level directory
/// matching the binstall `bin-dir` template.
///
/// # Errors
///
/// Returns [`InstallerPackagingError::BinaryNotFound`] if the binary does
/// not exist, or [`InstallerPackagingError::Io`] /
/// [`InstallerPackagingError::Zip`] on archive creation failures.
pub fn package_installer(
    params: InstallerPackageParams,
) -> Result<InstallerPackageOutput, InstallerPackagingError> {
    if !params.binary_path.is_file() {
        return Err(InstallerPackagingError::BinaryNotFound(
            params.binary_path.clone(),
        ));
    }

    let name = archive_filename(&params.version, &params.target);
    let output_path = params.output_dir.join(&name);
    let inner_dir = inner_dir_name(&params.version, &params.target);
    let bin_name = binary_filename(&params.target);

    match archive_format(&params.target) {
        ArchiveFormat::Tgz => {
            create_tgz_archive(&output_path, &inner_dir, &params.binary_path, &bin_name)?;
        }
        ArchiveFormat::Zip => {
            create_zip_archive(&output_path, &inner_dir, &params.binary_path, &bin_name)?;
        }
    }

    Ok(InstallerPackageOutput {
        archive_path: output_path,
        archive_name: name,
    })
}

/// Create a `.tgz` archive containing the binary inside a directory.
fn create_tgz_archive(
    output_path: &Path,
    inner_dir: &str,
    binary_path: &Path,
    bin_name: &str,
) -> Result<(), InstallerPackagingError> {
    let output_file = fs::File::create(output_path)?;
    let gz_encoder = flate2::write::GzEncoder::new(output_file, flate2::Compression::default());
    let mut archive = tar::Builder::new(gz_encoder);

    let archive_entry_path = format!("{inner_dir}/{bin_name}");
    archive.append_path_with_name(binary_path, &archive_entry_path)?;
    archive.finish()?;

    Ok(())
}

/// Create a `.zip` archive containing the binary inside a directory.
fn create_zip_archive(
    output_path: &Path,
    inner_dir: &str,
    binary_path: &Path,
    bin_name: &str,
) -> Result<(), InstallerPackagingError> {
    let output_file = fs::File::create(output_path)?;
    let mut zip_writer = zip::ZipWriter::new(output_file);

    let archive_entry_path = format!("{inner_dir}/{bin_name}");
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip_writer.start_file(&archive_entry_path, options)?;

    let mut binary_file = fs::File::open(binary_path)?;
    let mut buffer = [0u8; 8192];
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

#[cfg(test)]
#[path = "installer_packaging_tests.rs"]
mod tests;
