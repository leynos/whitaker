//! Installer orchestration for repository-hosted dependency binaries.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::artefact::target::TargetTriple;
use crate::dirs::BaseDirs;

use super::super::manifest::DependencyBinary;
use super::downloader::{DependencyArchiveDownloader, RepositoryArchiveDownloader};
use super::extractor::{DependencyArchiveExtractor, RepositoryArchiveExtractor};
use super::metadata::{archive_filename, expected_member_path};

/// Errors returned while installing repository-hosted dependency binaries.
#[derive(Debug, Error)]
pub enum DependencyBinaryInstallError {
    /// The platform executable directory could not be determined.
    #[error("could not determine local bin directory")]
    MissingBinDir,

    /// Downloading the archive failed.
    #[error("download failed for {url}: {reason}")]
    Download {
        /// The release asset URL that failed.
        url: String,
        /// Human-readable download failure details.
        reason: String,
    },

    /// Archive extraction failed.
    #[error("failed to extract {archive}: {reason}")]
    Extraction {
        /// Path to the archive that could not be extracted.
        archive: PathBuf,
        /// Human-readable extraction failure details.
        reason: String,
    },

    /// The archive did not contain the expected executable.
    #[error("archive did not contain expected binary {binary}")]
    MissingBinaryInArchive {
        /// The executable filename expected inside the archive.
        binary: String,
    },

    /// The extracted binary could not be installed locally.
    #[error("failed to install binary {binary}: {reason}")]
    Install {
        /// The executable that could not be installed locally.
        binary: String,
        /// Human-readable installation failure details.
        reason: String,
    },

    /// I/O failure while creating local directories or files.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Installs dependency binaries from repository-hosted release assets.
#[cfg_attr(test, mockall::automock)]
pub trait DependencyBinaryInstaller {
    /// Install one dependency binary for the current target into the local bin
    /// directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the asset cannot be downloaded, extracted, or
    /// written to disk.
    fn install(
        &self,
        dependency: &DependencyBinary,
        target: &TargetTriple,
        dirs: &dyn BaseDirs,
    ) -> Result<PathBuf, DependencyBinaryInstallError>;
}

/// Default repository installer using live downloads and archive extraction.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepositoryDependencyBinaryInstaller;

impl DependencyBinaryInstaller for RepositoryDependencyBinaryInstaller {
    fn install(
        &self,
        dependency: &DependencyBinary,
        target: &TargetTriple,
        dirs: &dyn BaseDirs,
    ) -> Result<PathBuf, DependencyBinaryInstallError> {
        install_with(
            dependency,
            target,
            &InstallSupport {
                dirs,
                downloader: &RepositoryArchiveDownloader,
                extractor: &RepositoryArchiveExtractor,
            },
        )
    }
}

/// Collaborators used by [`install_with`] to isolate directory, download, and
/// extraction behaviour in tests.
pub(crate) struct InstallSupport<'a> {
    pub(crate) dirs: &'a dyn BaseDirs,
    pub(crate) downloader: &'a dyn DependencyArchiveDownloader,
    pub(crate) extractor: &'a dyn DependencyArchiveExtractor,
}

/// Install one dependency binary using injected directory, download, and
/// extraction support.
pub(crate) fn install_with(
    dependency: &DependencyBinary,
    target: &TargetTriple,
    support: &InstallSupport<'_>,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let bin_dir = support
        .dirs
        .bin_dir()
        .ok_or(DependencyBinaryInstallError::MissingBinDir)?;
    fs::create_dir_all(bin_dir.as_path())?;

    let temp_dir = tempfile::tempdir()?;
    let filename = archive_filename(dependency, target);
    let archive_path = temp_dir.path().join(&filename);
    support.downloader.download(&filename, &archive_path)?;

    let member_path = expected_member_path(dependency, target);
    let installed_path =
        support
            .extractor
            .extract_binary(&archive_path, &member_path, bin_dir.as_path())?;
    ensure_executable(&installed_path)?;
    Ok(installed_path)
}

/// Apply executable permissions on platforms that require an explicit mode
/// change after extraction.
pub(crate) fn ensure_executable(path: &Path) -> Result<(), DependencyBinaryInstallError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|error| {
            DependencyBinaryInstallError::Install {
                binary: path.display().to_string(),
                reason: error.to_string(),
            }
        })?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
