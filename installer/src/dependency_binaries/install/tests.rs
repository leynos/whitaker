//! Unit tests for repository-hosted dependency-binary installation helpers.

use super::installer::{InstallSupport, install_with};
use super::*;
use crate::dirs::BaseDirs;
use crate::installer_packaging::TargetTriple;
use rstest::{fixture, rstest};
use std::cell::Cell;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

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
        expected_member_path: &str,
        destination_dir: &Path,
    ) -> Result<PathBuf, DependencyBinaryInstallError> {
        if !self.writes_binary {
            return Err(DependencyBinaryInstallError::MissingBinaryInArchive {
                binary: expected_member_path.to_owned(),
            });
        }
        let binary_name = Path::new(expected_member_path)
            .file_name()
            .expect("member path should include a filename");
        let path = destination_dir.join(binary_name);
        let mut file = File::create(&path)?;
        file.write_all(b"fake binary")?;
        self.extracted_path.set(Some(path.clone()));
        Ok(path)
    }
}

/// Build a deterministic installation setup for success and missing-binary
/// scenarios.
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
        .expect("dependency manifest should load")
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

#[fixture]
fn cargo_dylint_install_result() -> (
    tempfile::TempDir,
    PathBuf,
    Result<PathBuf, DependencyBinaryInstallError>,
) {
    run_install_scenario("cargo-dylint", true)
}

#[fixture]
fn missing_binary_install_result() -> (
    tempfile::TempDir,
    PathBuf,
    Result<PathBuf, DependencyBinaryInstallError>,
) {
    run_install_scenario("dylint-link", false)
}

#[test]
fn archive_filename_uses_dependency_version() {
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let dependency = crate::dependency_binaries::find_dependency_binary("cargo-dylint")
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    assert_eq!(
        archive_filename(dependency, &target),
        "cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0.tgz"
    );
}

#[test]
fn binary_filename_adds_windows_suffix() {
    let target = TargetTriple::try_from("x86_64-pc-windows-msvc").expect("valid target");
    let dependency = crate::dependency_binaries::find_dependency_binary("dylint-link")
        .expect("dependency manifest should load")
        .expect("dependency");
    assert_eq!(binary_filename(dependency, &target), "dylint-link.exe");
}

#[rstest]
fn install_with_creates_missing_bin_directory(
    cargo_dylint_install_result: (
        tempfile::TempDir,
        PathBuf,
        Result<PathBuf, DependencyBinaryInstallError>,
    ),
) {
    let (_temp_dir, bin_dir, result) = cargo_dylint_install_result;
    let installed_path = result.expect("install");
    assert!(bin_dir.is_dir());
    assert_eq!(installed_path, bin_dir.join("cargo-dylint"));
    assert!(installed_path.is_file());
}

#[rstest]
fn install_with_returns_error_when_binary_missing_after_extract(
    missing_binary_install_result: (
        tempfile::TempDir,
        PathBuf,
        Result<PathBuf, DependencyBinaryInstallError>,
    ),
) {
    let (_temp_dir, _bin_dir, result) = missing_binary_install_result;
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
