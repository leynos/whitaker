//! Installation helpers for repository-hosted dependency binaries.
//!
//! These helpers install `cargo-dylint` and `dylint-link` from Whitaker release
//! assets before the installer falls back to `cargo binstall` or `cargo
//! install`.

use super::manifest::DependencyBinary;
use crate::artefact::download::HttpDownloader;
use crate::artefact::target::TargetTriple;
use crate::dirs::BaseDirs;
use flate2::read::GzDecoder;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const ROLLING_TAG: &str = "rolling";
const GITHUB_REPO: &str = "leynos/whitaker";
const DOWNLOAD_TIMEOUT_SECS: u64 = 30;
const PROVENANCE_FILENAME: &str = "dependency-binaries-licences.md";

/// Return the current host target when Whitaker knows how to package it.
#[must_use]
pub fn host_target() -> Option<TargetTriple> {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    let target = "x86_64-unknown-linux-gnu";
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    let target = "aarch64-unknown-linux-gnu";
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    let target = "x86_64-apple-darwin";
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    let target = "aarch64-apple-darwin";
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    let target = "x86_64-pc-windows-msvc";
    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "windows"),
    )))]
    let target = "";

    if target.is_empty() {
        None
    } else {
        Some(TargetTriple::try_from(target).expect("supported target"))
    }
}

/// Return the release-side provenance asset filename.
#[must_use]
pub fn provenance_filename() -> &'static str {
    PROVENANCE_FILENAME
}

/// Compute the platform-specific executable name for a dependency binary.
#[must_use]
pub fn binary_filename(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    if target.is_windows() {
        format!("{}.exe", dependency.binary())
    } else {
        dependency.binary().to_owned()
    }
}

/// Compute the repository archive filename for a dependency binary.
#[must_use]
pub fn archive_filename(dependency: &DependencyBinary, target: &TargetTriple) -> String {
    let extension = if target.is_windows() { "zip" } else { "tgz" };
    format!(
        "{}-{}-v{}.{}",
        dependency.package(),
        target.as_str(),
        dependency.version(),
        extension
    )
}

/// Errors returned while installing repository-hosted dependency binaries.
#[derive(Debug, Error)]
pub enum DependencyBinaryInstallError {
    /// The platform executable directory could not be determined.
    #[error("could not determine local bin directory")]
    MissingBinDir,

    /// The platform executable directory was not UTF-8.
    #[error("local bin directory is not valid UTF-8: {0}")]
    NonUtf8BinDir(PathBuf),

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

/// Downloads dependency archives.
pub trait DependencyArchiveDownloader {
    /// Download `filename` into `destination`.
    ///
    /// # Errors
    ///
    /// Returns an error when the remote asset cannot be fetched.
    fn download(
        &self,
        filename: &str,
        destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError>;
}

/// Extracts a single executable from dependency archives.
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
        expected_binary_name: &str,
        destination_dir: &Path,
    ) -> Result<PathBuf, DependencyBinaryInstallError>;
}

/// Installs dependency binaries from repository-hosted release assets.
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

/// Production downloader for release archives.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepositoryArchiveDownloader;

impl DependencyArchiveDownloader for RepositoryArchiveDownloader {
    fn download(
        &self,
        filename: &str,
        destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError> {
        let url = asset_url(filename);
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS)))
            .build();
        let response = ureq::Agent::new_with_config(config)
            .get(&url)
            .call()
            .map_err(|error| DependencyBinaryInstallError::Download {
                url: url.clone(),
                reason: error.to_string(),
            })?;
        let mut file = File::create(destination)?;
        let mut body = response.into_body();
        let mut reader = body.as_reader();
        io::copy(&mut reader, &mut file)?;
        Ok(())
    }
}

/// Production extractor for `.tgz` and `.zip` archives.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepositoryArchiveExtractor;

impl DependencyArchiveExtractor for RepositoryArchiveExtractor {
    fn extract_binary(
        &self,
        archive_path: &Path,
        expected_binary_name: &str,
        destination_dir: &Path,
    ) -> Result<PathBuf, DependencyBinaryInstallError> {
        if archive_path
            .extension()
            .is_some_and(|extension| extension == "zip")
        {
            return extract_from_zip(archive_path, expected_binary_name, destination_dir);
        }
        extract_from_tgz(archive_path, expected_binary_name, destination_dir)
    }
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

pub(crate) struct InstallSupport<'a> {
    pub(crate) dirs: &'a dyn BaseDirs,
    pub(crate) downloader: &'a dyn DependencyArchiveDownloader,
    pub(crate) extractor: &'a dyn DependencyArchiveExtractor,
}

pub(crate) fn install_with(
    dependency: &DependencyBinary,
    target: &TargetTriple,
    support: &InstallSupport<'_>,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let bin_dir = support
        .dirs
        .bin_dir()
        .ok_or(DependencyBinaryInstallError::MissingBinDir)?;
    let bin_dir = camino::Utf8PathBuf::from_path_buf(bin_dir)
        .map_err(DependencyBinaryInstallError::NonUtf8BinDir)?;
    fs::create_dir_all(bin_dir.as_std_path())?;

    let temp_dir = tempfile::tempdir()?;
    let filename = archive_filename(dependency, target);
    let archive_path = temp_dir.path().join(&filename);
    support.downloader.download(&filename, &archive_path)?;

    let binary_name = binary_filename(dependency, target);
    let installed_path =
        support
            .extractor
            .extract_binary(&archive_path, &binary_name, bin_dir.as_std_path())?;
    ensure_executable(&installed_path)?;
    Ok(installed_path)
}

fn extract_from_tgz(
    archive_path: &Path,
    expected_binary_name: &str,
    destination_dir: &Path,
) -> Result<PathBuf, DependencyBinaryInstallError> {
    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|error| DependencyBinaryInstallError::Extraction {
            archive: archive_path.to_path_buf(),
            reason: error.to_string(),
        })?
    {
        let mut entry = entry.map_err(|error| DependencyBinaryInstallError::Extraction {
            archive: archive_path.to_path_buf(),
            reason: error.to_string(),
        })?;
        let path = entry
            .path()
            .map_err(|error| DependencyBinaryInstallError::Extraction {
                archive: archive_path.to_path_buf(),
                reason: error.to_string(),
            })?
            .into_owned();
        if path
            .file_name()
            .is_some_and(|name| name == expected_binary_name)
        {
            let destination = destination_dir.join(expected_binary_name);
            let mut output = File::create(&destination)?;
            io::copy(&mut entry, &mut output)?;
            return Ok(destination);
        }
    }

    Err(DependencyBinaryInstallError::MissingBinaryInArchive {
        binary: expected_binary_name.to_owned(),
    })
}

fn extract_from_zip(
    archive_path: &Path,
    expected_binary_name: &str,
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
        let Some(name) = Path::new(file.name()).file_name() else {
            continue;
        };
        if name != expected_binary_name {
            continue;
        }
        let destination = destination_dir.join(expected_binary_name);
        let mut output = File::create(&destination)?;
        io::copy(&mut file, &mut output)?;
        return Ok(destination);
    }

    Err(DependencyBinaryInstallError::MissingBinaryInArchive {
        binary: expected_binary_name.to_owned(),
    })
}

fn asset_url(filename: &str) -> String {
    let _ = HttpDownloader::asset_url(filename);
    format!("https://github.com/{GITHUB_REPO}/releases/download/{ROLLING_TAG}/{filename}")
}

fn ensure_executable(path: &Path) -> Result<(), DependencyBinaryInstallError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dirs::BaseDirs;
    use std::cell::Cell;
    use std::io::Write;

    struct StubDirs {
        bin_dir: Option<PathBuf>,
    }

    impl BaseDirs for StubDirs {
        fn home_dir(&self) -> Option<PathBuf> {
            None
        }

        fn bin_dir(&self) -> Option<PathBuf> {
            self.bin_dir.clone()
        }

        fn whitaker_data_dir(&self) -> Option<PathBuf> {
            None
        }
    }

    struct StubDownloader {
        content: Vec<u8>,
        fail: bool,
    }

    impl DependencyArchiveDownloader for StubDownloader {
        fn download(
            &self,
            _filename: &str,
            destination: &Path,
        ) -> Result<(), DependencyBinaryInstallError> {
            if self.fail {
                return Err(DependencyBinaryInstallError::Download {
                    url: "https://example.test/archive".to_owned(),
                    reason: "download failed".to_owned(),
                });
            }
            fs::write(destination, &self.content)?;
            Ok(())
        }
    }

    struct StubExtractor {
        writes_binary: bool,
        extracted_path: Cell<Option<PathBuf>>,
    }

    impl DependencyArchiveExtractor for StubExtractor {
        fn extract_binary(
            &self,
            _archive_path: &Path,
            expected_binary_name: &str,
            destination_dir: &Path,
        ) -> Result<PathBuf, DependencyBinaryInstallError> {
            if !self.writes_binary {
                return Err(DependencyBinaryInstallError::MissingBinaryInArchive {
                    binary: expected_binary_name.to_owned(),
                });
            }
            let path = destination_dir.join(expected_binary_name);
            let mut file = File::create(&path)?;
            file.write_all(b"fake binary")?;
            self.extracted_path.set(Some(path.clone()));
            Ok(path)
        }
    }

    fn run_install_scenario(
        dependency_name: &str,
        writes_binary: bool,
    ) -> (
        tempfile::TempDir,
        PathBuf,
        Result<PathBuf, DependencyBinaryInstallError>,
    ) {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        let dirs = StubDirs {
            bin_dir: Some(bin_dir.clone()),
        };
        let dependency = crate::dependency_binaries::find_dependency_binary(dependency_name)
            .expect("dependency should exist");
        let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
        let downloader = StubDownloader {
            content: b"archive".to_vec(),
            fail: false,
        };
        let extractor = StubExtractor {
            writes_binary,
            extracted_path: Cell::new(None),
        };
        let result = install_with(
            dependency,
            &target,
            &InstallSupport {
                dirs: &dirs,
                downloader: &downloader,
                extractor: &extractor,
            },
        );

        (temp_dir, bin_dir, result)
    }

    #[test]
    fn archive_filename_uses_dependency_version() {
        let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
        let dependency = crate::dependency_binaries::find_dependency_binary("cargo-dylint")
            .expect("dependency should exist");
        assert_eq!(
            archive_filename(dependency, &target),
            "cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0.tgz"
        );
    }

    #[test]
    fn binary_filename_adds_windows_suffix() {
        let target = TargetTriple::try_from("x86_64-pc-windows-msvc").expect("valid target");
        let dependency =
            crate::dependency_binaries::find_dependency_binary("dylint-link").expect("dependency");
        assert_eq!(binary_filename(dependency, &target), "dylint-link.exe");
    }

    #[test]
    fn install_with_creates_missing_bin_directory() {
        let (_temp_dir, bin_dir, result) = run_install_scenario("cargo-dylint", true);
        let installed_path = result.expect("install");
        assert!(bin_dir.is_dir());
        assert_eq!(installed_path, bin_dir.join("cargo-dylint"));
        assert!(installed_path.is_file());
    }

    #[test]
    fn install_with_returns_error_when_binary_missing_after_extract() {
        let (_temp_dir, _bin_dir, result) = run_install_scenario("dylint-link", false);
        let error = result.expect_err("install should fail");
        assert!(matches!(
            error,
            DependencyBinaryInstallError::MissingBinaryInArchive { .. }
        ));
    }

    #[test]
    fn provenance_filename_is_stable() {
        assert_eq!(provenance_filename(), "dependency-binaries-licences.md");
    }
}
