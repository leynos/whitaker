//! Unit tests for installer binary archive packaging.

use super::*;
use crate::binstall_metadata;
use rstest::rstest;
use std::fs;
use std::io::Read;

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

/// A temporary directory with a fake binary suitable for packaging.
struct PackagingFixture {
    temp: tempfile::TempDir,
    binary_path: std::path::PathBuf,
}

/// Create a [`PackagingFixture`] for the given target, writing a fake binary
/// with the provided content into a fresh temporary directory.
fn packaging_fixture(target: &str, content: &[u8]) -> PackagingFixture {
    let temp = tempfile::tempdir().expect("temp dir");
    let bin_name = binary_filename(&TargetTriple::new(target));
    let binary_path = temp.path().join(bin_name);
    fs::write(&binary_path, content).expect("write fake binary");
    PackagingFixture { temp, binary_path }
}

/// Build [`InstallerPackageParams`] from a fixture, version, and target.
fn params_from_fixture(
    fixture: &PackagingFixture,
    version: &str,
    target: &str,
) -> InstallerPackageParams {
    InstallerPackageParams {
        version: Version::new(version),
        target: TargetTriple::new(target),
        binary_path: fixture.binary_path.clone(),
        output_dir: fixture.temp.path().to_path_buf(),
    }
}

/// Read entry paths from a `.tgz` archive, failing explicitly on errors.
fn read_tgz_entry_paths(archive_path: &std::path::Path) -> Vec<String> {
    let file = fs::File::open(archive_path).expect("open archive");
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar_archive = tar::Archive::new(gz);
    tar_archive
        .entries()
        .expect("entries")
        .map(|e| {
            let entry = e.expect("valid tar entry");
            entry
                .path()
                .expect("valid entry path")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Pure function tests
// ---------------------------------------------------------------------------

#[rstest]
#[case::linux_x86("x86_64-unknown-linux-gnu")]
#[case::linux_arm("aarch64-unknown-linux-gnu")]
#[case::macos_x86("x86_64-apple-darwin")]
#[case::macos_arm("aarch64-apple-darwin")]
fn archive_filename_tgz_for_non_windows(#[case] target: &str) {
    let v = Version::new("0.2.1");
    let t = TargetTriple::new(target);
    let name = archive_filename(&v, &t);
    assert!(name.ends_with(".tgz"), "expected .tgz suffix, got {name}");
    assert!(name.contains(target), "expected target in name, got {name}");
    assert!(
        name.contains("v0.2.1"),
        "expected version in name, got {name}"
    );
}

#[test]
fn archive_filename_zip_for_windows() {
    let v = Version::new("0.2.1");
    let t = TargetTriple::new("x86_64-pc-windows-msvc");
    let name = archive_filename(&v, &t);
    assert_eq!(name, "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1.zip");
}

#[rstest]
#[case::linux(
    "x86_64-unknown-linux-gnu",
    "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1"
)]
#[case::macos(
    "aarch64-apple-darwin",
    "whitaker-installer-aarch64-apple-darwin-v0.2.1"
)]
#[case::windows(
    "x86_64-pc-windows-msvc",
    "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1"
)]
fn inner_dir_name_matches_expected(#[case] target: &str, #[case] expected: &str) {
    assert_eq!(
        inner_dir_name(&Version::new("0.2.1"), &TargetTriple::new(target)),
        expected
    );
}

#[rstest]
#[case::linux("x86_64-unknown-linux-gnu")]
#[case::macos("aarch64-apple-darwin")]
fn binary_filename_unix(#[case] target: &str) {
    assert_eq!(
        binary_filename(&TargetTriple::new(target)),
        "whitaker-installer"
    );
}

#[test]
fn binary_filename_windows() {
    assert_eq!(
        binary_filename(&TargetTriple::new("x86_64-pc-windows-msvc")),
        "whitaker-installer.exe"
    );
}

#[rstest]
#[case::linux_x86("x86_64-unknown-linux-gnu")]
#[case::linux_arm("aarch64-unknown-linux-gnu")]
#[case::macos_x86("x86_64-apple-darwin")]
#[case::macos_arm("aarch64-apple-darwin")]
fn archive_format_tgz_for_non_windows(#[case] target: &str) {
    assert_eq!(
        archive_format(&TargetTriple::new(target)),
        ArchiveFormat::Tgz
    );
}

#[test]
fn archive_format_zip_for_windows() {
    assert_eq!(
        archive_format(&TargetTriple::new("x86_64-pc-windows-msvc")),
        ArchiveFormat::Zip
    );
}

// ---------------------------------------------------------------------------
// Archive creation tests
// ---------------------------------------------------------------------------

#[test]
fn package_installer_creates_tgz() {
    let fixture = packaging_fixture("x86_64-unknown-linux-gnu", b"fake-binary-content");
    let params = params_from_fixture(&fixture, "0.2.1", "x86_64-unknown-linux-gnu");

    let output = package_installer(params).expect("packaging should succeed");
    assert!(output.archive_path.exists(), "archive should exist");
    assert_eq!(
        output.archive_name,
        "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1.tgz"
    );

    let entries = read_tgz_entry_paths(&output.archive_path);
    assert_eq!(entries.len(), 1, "expected 1 entry, got {entries:?}");
    assert_eq!(
        entries[0],
        "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1/whitaker-installer"
    );
}

#[test]
fn package_installer_creates_zip() {
    let fixture = packaging_fixture("x86_64-pc-windows-msvc", b"fake-exe-content");
    let params = params_from_fixture(&fixture, "0.2.1", "x86_64-pc-windows-msvc");

    let output = package_installer(params).expect("packaging should succeed");
    assert!(output.archive_path.exists(), "archive should exist");
    assert_eq!(
        output.archive_name,
        "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1.zip"
    );

    // Verify archive contents
    let file = fs::File::open(&output.archive_path).expect("open archive");
    let mut zip_archive = zip::ZipArchive::new(file).expect("open zip");
    assert_eq!(zip_archive.len(), 1, "expected 1 entry in zip");

    let entry = zip_archive.by_index(0).expect("first entry");
    assert_eq!(
        entry.name(),
        "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1/whitaker-installer.exe"
    );
}

#[test]
fn package_installer_rejects_missing_binary() {
    let temp = tempfile::tempdir().expect("temp dir");
    let missing = temp.path().join("does-not-exist");

    let params = InstallerPackageParams {
        version: Version::new("0.2.1"),
        target: TargetTriple::new("x86_64-unknown-linux-gnu"),
        binary_path: missing.clone(),
        output_dir: temp.path().to_path_buf(),
    };

    let err = package_installer(params).expect_err("should fail");
    assert!(
        matches!(err, InstallerPackagingError::BinaryNotFound(ref p) if *p == missing),
        "expected BinaryNotFound, got {err:?}"
    );
}

#[test]
fn package_installer_returns_io_error_for_unwritable_output() {
    let temp = tempfile::tempdir().expect("temp dir");
    let binary = temp.path().join("whitaker-installer");
    fs::write(&binary, b"content").expect("write");

    // Use a path under /dev/null (Linux) which cannot be a directory.
    let unwritable = std::path::PathBuf::from("/dev/null/impossible");

    let params = InstallerPackageParams {
        version: Version::new("0.2.1"),
        target: TargetTriple::new("x86_64-unknown-linux-gnu"),
        binary_path: binary,
        output_dir: unwritable,
    };

    let err = package_installer(params).expect_err("should fail on unwritable output dir");
    assert!(
        matches!(err, InstallerPackagingError::Io(_)),
        "expected Io error, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Cross-validation with binstall metadata
// ---------------------------------------------------------------------------

#[rstest]
#[case::linux("x86_64-unknown-linux-gnu", "0.2.1")]
#[case::windows("x86_64-pc-windows-msvc", "0.2.1")]
#[case::macos_arm("aarch64-apple-darwin", "1.0.0")]
fn archive_name_matches_binstall_template(#[case] target: &str, #[case] version: &str) {
    let v = Version::new(version);
    let t = TargetTriple::new(target);
    let name = archive_filename(&v, &t);
    let url = binstall_metadata::expand_pkg_url(version, target);
    assert!(
        url.ends_with(&name),
        "expected binstall URL to end with archive filename\n  URL:  {url}\n  name: {name}"
    );
}

// ---------------------------------------------------------------------------
// Tgz archive content verification
// ---------------------------------------------------------------------------

#[test]
fn tgz_archive_preserves_binary_content() {
    let content = b"binary-payload-12345";
    let fixture = packaging_fixture("aarch64-unknown-linux-gnu", content);
    let params = params_from_fixture(&fixture, "0.2.1", "aarch64-unknown-linux-gnu");

    let output = package_installer(params).expect("packaging");
    let file = fs::File::open(&output.archive_path).expect("open");
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar_archive = tar::Archive::new(gz);

    let mut entry = tar_archive
        .entries()
        .expect("entries")
        .next()
        .expect("one entry")
        .expect("valid entry");
    let mut extracted = Vec::new();
    entry.read_to_end(&mut extracted).expect("read");
    assert_eq!(extracted.as_slice(), content);
}
