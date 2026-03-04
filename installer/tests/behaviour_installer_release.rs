//! Behaviour-driven tests for installer release archive packaging.
//!
//! These scenarios verify that the `installer_packaging` module produces
//! archives matching the binstall `pkg-url` and `bin-dir` templates for
//! all supported targets, and that error paths are handled correctly.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::path::PathBuf;
use whitaker_installer::binstall_metadata;
use whitaker_installer::installer_packaging::{
    self, ArchiveFormat, InstallerPackageOutput, InstallerPackageParams, InstallerPackagingError,
    TargetTriple, Version,
};

// ---------------------------------------------------------------------------
// World type
// ---------------------------------------------------------------------------

/// Mutable state threaded through Gherkin steps.
#[derive(Default)]
struct InstallerReleaseWorld {
    version: String,
    target: String,
    computed_filename: String,
    temp_dir: Option<tempfile::TempDir>,
    binary_path: Option<PathBuf>,
    package_output: Option<InstallerPackageOutput>,
    packaging_error: Option<InstallerPackagingError>,
}

#[fixture]
fn world() -> InstallerReleaseWorld {
    InstallerReleaseWorld::default()
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("version \"{version}\" and target \"{target}\"")]
fn given_version_and_target(world: &mut InstallerReleaseWorld, version: String, target: String) {
    world.version = version;
    world.target = target;
}

#[given("a fake installer binary exists")]
fn given_fake_binary_exists(world: &mut InstallerReleaseWorld) {
    let temp = tempfile::tempdir().expect("temp dir");
    let bin_name = installer_packaging::binary_filename(&TargetTriple::new(&world.target));
    let binary_path = temp.path().join(&bin_name);
    std::fs::write(&binary_path, b"fake-binary").expect("write fake binary");
    world.binary_path = Some(binary_path);
    world.temp_dir = Some(temp);
}

#[given("the binary path does not exist")]
fn given_binary_missing(world: &mut InstallerReleaseWorld) {
    let temp = tempfile::tempdir().expect("temp dir");
    world.binary_path = Some(temp.path().join("does-not-exist"));
    world.temp_dir = Some(temp);
}

#[when("the archive filename is computed")]
fn when_archive_filename_computed(world: &mut InstallerReleaseWorld) {
    world.computed_filename = installer_packaging::archive_filename(
        &Version::new(&world.version),
        &TargetTriple::new(&world.target),
    );
}

/// Run the packaging pipeline and store the result in the world.
fn attempt_packaging(world: &mut InstallerReleaseWorld) {
    let temp_dir = world.temp_dir.as_ref().expect("temp dir set");
    let binary_path = world.binary_path.as_ref().expect("binary path set");

    let params = InstallerPackageParams {
        version: Version::new(&world.version),
        target: TargetTriple::new(&world.target),
        binary_path: binary_path.clone(),
        output_dir: temp_dir.path().to_path_buf(),
    };

    match installer_packaging::package_installer(params) {
        Ok(output) => world.package_output = Some(output),
        Err(e) => world.packaging_error = Some(e),
    }
}

#[when("the installer is packaged")]
fn when_installer_packaged(world: &mut InstallerReleaseWorld) {
    attempt_packaging(world);
}

#[when("packaging is attempted")]
fn when_packaging_attempted(world: &mut InstallerReleaseWorld) {
    attempt_packaging(world);
}

#[then("the archive filename is \"{expected}\"")]
fn then_archive_filename_is(world: &mut InstallerReleaseWorld, expected: String) {
    assert_eq!(
        world.computed_filename, expected,
        "archive filename mismatch"
    );
}

#[then("the archive contains \"{expected_path}\"")]
fn then_archive_contains(world: &mut InstallerReleaseWorld, expected_path: String) {
    let output = world
        .package_output
        .as_ref()
        .expect("package output should be set");

    let format = installer_packaging::archive_format(&TargetTriple::new(&world.target));
    let entries = read_archive_entries(&output.archive_path, format);

    assert!(
        entries.contains(&expected_path),
        "expected archive to contain '{expected_path}', found: {entries:?}"
    );
}

#[then("the binstall pkg-url ends with the archive filename")]
fn then_binstall_url_ends_with_filename(world: &mut InstallerReleaseWorld) {
    let url = binstall_metadata::expand_pkg_url(&world.version, &world.target);
    let filename = &world.computed_filename;
    assert!(
        url.ends_with(filename.as_str()),
        "expected URL to end with '{filename}', got '{url}'"
    );
}

#[then("a packaging error is returned")]
fn then_packaging_error_returned(world: &mut InstallerReleaseWorld) {
    let err = world
        .packaging_error
        .as_ref()
        .expect("expected packaging to fail, but it succeeded");
    assert!(
        matches!(err, InstallerPackagingError::BinaryNotFound(_)),
        "expected BinaryNotFound, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read entry paths from an archive file.
fn read_archive_entries(path: &std::path::Path, format: ArchiveFormat) -> Vec<String> {
    match format {
        ArchiveFormat::Tgz => read_tgz_entries(path),
        ArchiveFormat::Zip => read_zip_entries(path),
    }
}

/// Read entry paths from a `.tgz` archive.
fn read_tgz_entries(path: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(path).expect("open tgz");
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive
        .entries()
        .expect("entries")
        .filter_map(|e| {
            e.ok()
                .and_then(|entry| entry.path().ok().map(|p| p.to_string_lossy().into_owned()))
        })
        .collect()
}

/// Read entry paths from a `.zip` archive.
fn read_zip_entries(path: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(path).expect("open zip");
    let archive = zip::ZipArchive::new(file).expect("open zip archive");
    (0..archive.len())
        .map(|i| {
            let entry = archive.name_for_index(i).expect("entry name");
            entry.to_owned()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Archive filename uses tgz for Linux target"
)]
fn scenario_archive_filename_tgz(world: InstallerReleaseWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Archive filename uses zip for Windows target"
)]
fn scenario_archive_filename_zip(world: InstallerReleaseWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Archive contains correct directory structure for Unix"
)]
fn scenario_archive_structure_unix(world: InstallerReleaseWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Windows archive contains exe binary"
)]
fn scenario_archive_structure_windows(world: InstallerReleaseWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Archive filename matches binstall pkg-url template"
)]
fn scenario_binstall_url_match(world: InstallerReleaseWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/installer_release.feature",
    name = "Packaging rejects missing binary"
)]
fn scenario_packaging_rejects_missing(world: InstallerReleaseWorld) {
    let _ = world;
}
