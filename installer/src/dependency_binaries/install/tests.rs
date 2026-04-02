//! Unit tests for repository-hosted dependency-binary installation helpers.

use super::downloader::MockDependencyArchiveDownloader;
use super::extractor::MockDependencyArchiveExtractor;
use super::installer::{InstallSupport, install_with};
use super::metadata::expected_member_path;
use super::{archive_filename, *};
use crate::dirs::MockBaseDirs;
use crate::installer_packaging::TargetTriple;
use mockall::predicate::{always, eq};
use rstest::{fixture, rstest};
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut dirs = MockBaseDirs::new();
    dirs.expect_bin_dir()
        .once()
        .return_const(Some(bin_dir.clone()));
    let dependency = crate::dependency_binaries::find_dependency_binary(dependency_name)
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let mut downloader = MockDependencyArchiveDownloader::new();
    let expected_archive = archive_filename(dependency, &target);
    downloader
        .expect_download()
        .once()
        .with(eq(expected_archive), always())
        .returning(|_, destination| {
            fs::write(destination, b"archive")?;
            Ok(())
        });
    let mut extractor = MockDependencyArchiveExtractor::new();
    extractor
        .expect_extract_binary()
        .once()
        .with(always(), always(), always())
        .returning(move |_, expected_member_path, destination_dir| {
            if !writes_binary {
                return Err(DependencyBinaryInstallError::MissingBinaryInArchive {
                    binary: expected_member_path.to_owned(),
                });
            }
            let binary_name = Path::new(expected_member_path)
                .file_name()
                .expect("member path should include a filename");
            let path = destination_dir.join(binary_name);
            fs::write(&path, b"fake binary")?;
            Ok(path)
        });
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(&installed_path).expect("metadata");
        assert_ne!(metadata.permissions().mode() & 0o111, 0);
    }
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
    let dependency = crate::dependency_binaries::find_dependency_binary("dylint-link")
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let expected_path = expected_member_path(dependency, &target);
    let error = result.expect_err("install should fail");
    match error {
        DependencyBinaryInstallError::MissingBinaryInArchive { binary } => {
            assert_eq!(binary, expected_path);
        }
        other => panic!("expected MissingBinaryInArchive, got {:?}", other),
    }
}

#[test]
fn provenance_filename_is_stable() {
    assert_eq!(provenance_filename(), "dependency-binaries-licences.md");
}
