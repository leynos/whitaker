//! Archive extraction helpers for repository-hosted dependency binaries.

use super::installer::DependencyBinaryInstallError;
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Extracts a single executable from dependency archives.
#[cfg_attr(test, mockall::automock)]
pub trait DependencyArchiveExtractor {
    /// Extract the expected executable into `destination_dir`.
    ///
    /// # Errors
    ///
    /// Returns an error when the archive format is unreadable or the expected
    /// executable is missing.
    fn extract_binary(
        &self,
        archive_path: &Path,
        expected_member_path: &str,
        destination_dir: &Path,
    ) -> Result<PathBuf, DependencyBinaryInstallError>;
}

/// Production extractor for `.tgz` and `.zip` archives.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RepositoryArchiveExtractor;

impl DependencyArchiveExtractor for RepositoryArchiveExtractor {
    fn extract_binary(
        &self,
        archive_path: &Path,
        expected_member_path: &str,
        destination_dir: &Path,
    ) -> Result<PathBuf, DependencyBinaryInstallError> {
        if archive_path
            .extension()
            .is_some_and(|extension| extension == "zip")
        {
            return extract_from_zip(archive_path, expected_member_path, destination_dir);
        }
        extract_from_tgz(archive_path, expected_member_path, destination_dir)
    }
}

/// Extract the expected executable from a `.tgz` archive into `destination_dir`.
pub(crate) fn extract_from_tgz(
    archive_path: &Path,
    expected_member_path: &str,
    destination_dir: &Path,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let file = File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let map_archive_err = |error: io::Error| DependencyBinaryInstallError::Extraction {
        archive: archive_path.to_path_buf(),
        reason: error.to_string(),
    };
    for entry in archive.entries().map_err(map_archive_err)? {
        let mut entry = entry.map_err(map_archive_err)?;
        let path = entry.path().map_err(map_archive_err)?.into_owned();
        if path == Path::new(expected_member_path) {
            return extract_entry_to_destination(&mut entry, expected_member_path, destination_dir);
        }
    }

    Err(DependencyBinaryInstallError::MissingBinaryInArchive {
        binary: expected_member_path.to_owned(),
    })
}

/// Extract the expected executable from a ZIP archive into `destination_dir`.
pub(crate) fn extract_from_zip(
    archive_path: &Path,
    expected_member_path: &str,
    destination_dir: &Path,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let file = File::open(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| DependencyBinaryInstallError::Extraction {
            archive: archive_path.to_path_buf(),
            reason: error.to_string(),
        })?;

    for index in 0..archive.len() {
        let mut file =
            archive
                .by_index(index)
                .map_err(|error| DependencyBinaryInstallError::Extraction {
                    archive: archive_path.to_path_buf(),
                    reason: error.to_string(),
                })?;
        if file.name() != expected_member_path {
            continue;
        }
        return extract_entry_to_destination(&mut file, expected_member_path, destination_dir);
    }

    Err(DependencyBinaryInstallError::MissingBinaryInArchive {
        binary: expected_member_path.to_owned(),
    })
}

/// Write a matched archive member to a temporary file and rename it atomically.
fn extract_entry_to_destination(
    reader: &mut dyn Read,
    expected_member_path: &str,
    destination_dir: &Path,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let binary_name = Path::new(expected_member_path).file_name().ok_or_else(|| {
        DependencyBinaryInstallError::MissingBinaryInArchive {
            binary: expected_member_path.to_owned(),
        }
    })?;
    let destination = destination_dir.join(binary_name);
    let temporary_destination =
        destination_dir.join(format!(".tmp_{}", binary_name.to_string_lossy()));
    let write_result = write_entry(reader, &temporary_destination);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_destination);
        return Err(error);
    }
    fs::rename(&temporary_destination, &destination)?;
    Ok(destination)
}

/// Stream one archive member into a temporary file on disk.
fn write_entry(
    reader: &mut dyn Read,
    temporary_destination: &Path,
) -> Result<(), DependencyBinaryInstallError> {
    let mut output = File::create(temporary_destination)?;
    io::copy(reader, &mut output)?;
    output.flush()?;
    Ok(())
}
