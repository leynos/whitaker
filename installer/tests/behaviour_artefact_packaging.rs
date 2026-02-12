//! Behaviour-driven tests for artefact packaging.
//!
//! These scenarios validate the packaging pipeline defined in the
//! `artefact::packaging` module against ADR-001 rules. Tests use the
//! rstest-bdd v0.5.0 mutable world pattern.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use whitaker_installer::artefact::git_sha::GitSha;
use whitaker_installer::artefact::manifest::GeneratedAt;
use whitaker_installer::artefact::naming::ArtefactName;
use whitaker_installer::artefact::packaging::{
    PackageOutput, PackageParams, compute_sha256, package_artefact,
};
use whitaker_installer::artefact::packaging_error::PackagingError;
use whitaker_installer::artefact::target::TargetTriple;
use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;

// ---------------------------------------------------------------------------
// World types
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PackagingWorld {
    temp_dir: Option<TempDir>,
    library_files: Vec<PathBuf>,
    git_sha: Option<GitSha>,
    toolchain: Option<ToolchainChannel>,
    target: Option<TargetTriple>,
    output: Option<PackageOutput>,
    packaging_error: Option<PackagingError>,
    manifest_json: Option<serde_json::Value>,
    archive_sha256: Option<String>,
}

#[fixture]
fn world() -> PackagingWorld {
    PackagingWorld {
        temp_dir: Some(TempDir::new().expect("temp dir")),
        ..PackagingWorld::default()
    }
}

/// Return the temp directory path, creating one if needed.
fn temp_path(world: &PackagingWorld) -> PathBuf {
    world
        .temp_dir
        .as_ref()
        .expect("temp_dir set")
        .path()
        .to_path_buf()
}

/// Run the packaging pipeline and store the result in the world.
fn run_packaging(world: &mut PackagingWorld) {
    let output_dir = temp_path(world).join("dist");
    fs::create_dir_all(&output_dir).expect("mkdir dist");

    let params = PackageParams {
        git_sha: world.git_sha.clone().expect("git_sha set"),
        toolchain: world.toolchain.clone().expect("toolchain set"),
        target: world.target.clone().expect("target set"),
        library_files: world.library_files.clone(),
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T00:00:00Z"),
    };

    match package_artefact(params) {
        Ok(output) => world.output = Some(output),
        Err(e) => world.packaging_error = Some(e),
    }
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("a library file \"{name}\"")]
fn given_library_file(world: &mut PackagingWorld, name: String) {
    let path = temp_path(world).join(&name);
    fs::write(&path, b"fake library content").expect("write lib");
    world.library_files.push(path);
}

#[given("a git SHA \"{sha}\"")]
fn given_git_sha(world: &mut PackagingWorld, sha: String) {
    world.git_sha = Some(GitSha::try_from(sha).expect("valid SHA"));
}

#[given("a toolchain channel \"{channel}\"")]
fn given_toolchain(world: &mut PackagingWorld, channel: String) {
    world.toolchain = Some(ToolchainChannel::try_from(channel).expect("valid channel"));
}

#[given("a target triple \"{triple}\"")]
fn given_target(world: &mut PackagingWorld, triple: String) {
    world.target = Some(TargetTriple::try_from(triple).expect("valid target"));
}

#[when("the artefact is packaged")]
fn when_packaged(world: &mut PackagingWorld) {
    run_packaging(world);
}

#[then("the archive exists with the expected ADR-001 filename")]
fn then_archive_exists(world: &mut PackagingWorld) {
    let output = world.output.as_ref().expect("output set");
    assert!(output.archive_path.exists(), "archive file must exist");
    let filename = output
        .archive_path
        .file_name()
        .expect("filename")
        .to_string_lossy();
    assert!(
        filename.starts_with("whitaker-lints-"),
        "filename must start with 'whitaker-lints-'"
    );
    assert!(
        filename.ends_with(".tar.zst"),
        "filename must end with '.tar.zst'"
    );
}

#[then("the archive contains the library file")]
fn then_archive_has_library(world: &mut PackagingWorld) {
    let entries = list_archive_entries(world);
    assert!(
        entries.iter().any(|e| e.ends_with(".so")),
        "archive must contain a .so file"
    );
}

#[then("the archive contains a manifest.json")]
fn then_archive_has_manifest(world: &mut PackagingWorld) {
    let entries = list_archive_entries(world);
    assert!(
        entries.contains(&"manifest.json".to_owned()),
        "archive must contain manifest.json"
    );
}

#[given("a packaged artefact")]
fn given_packaged_artefact(world: &mut PackagingWorld) {
    let path = temp_path(world).join("libwhitaker_suite.so");
    fs::write(&path, b"fake library").expect("write");
    world.library_files.push(path);
    world.git_sha = Some(GitSha::try_from("abc1234").expect("valid"));
    world.toolchain = Some(ToolchainChannel::try_from("nightly-2025-09-18").expect("valid"));
    world.target = Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"));
    run_packaging(world);
}

#[when("the manifest is extracted")]
fn when_manifest_extracted(world: &mut PackagingWorld) {
    let output = world.output.as_ref().expect("output set");
    let file = fs::File::open(&output.archive_path).expect("open");
    let decoder = zstd::Decoder::new(file).expect("decode");
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().expect("entries") {
        let mut entry = entry.expect("entry");
        let path = entry.path().expect("path").to_string_lossy().into_owned();
        if path == "manifest.json" {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents).expect("read manifest");
            world.manifest_json = Some(serde_json::from_str(&contents).expect("parse JSON"));
            return;
        }
    }
    panic!("manifest.json not found in archive");
}

#[then("the manifest contains field \"{field}\"")]
fn then_manifest_has_field(world: &mut PackagingWorld, field: String) {
    let json = world.manifest_json.as_ref().expect("manifest_json set");
    let obj = json.as_object().expect("top-level object");
    assert!(obj.contains_key(&field), "missing field: {field}");
}

#[when("the archive SHA-256 is computed")]
fn when_sha256_computed(world: &mut PackagingWorld) {
    let output = world.output.as_ref().expect("output set");
    let digest = compute_sha256(&output.archive_path).expect("sha256");
    world.archive_sha256 = Some(digest.as_str().to_owned());
}

#[then("it is a valid 64-character hex string")]
fn then_valid_hex(world: &mut PackagingWorld) {
    let hex = world.archive_sha256.as_ref().expect("sha256 set");
    assert_eq!(hex.len(), 64, "digest must be 64 characters");
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit()),
        "digest must be hex"
    );
}

#[given("no library files")]
fn given_no_files(world: &mut PackagingWorld) {
    world.library_files.clear();
    world.git_sha = Some(GitSha::try_from("abc1234").expect("valid"));
    world.toolchain = Some(ToolchainChannel::try_from("nightly-2025-09-18").expect("valid"));
    world.target = Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"));
}

#[when("packaging is attempted")]
fn when_packaging_attempted(world: &mut PackagingWorld) {
    run_packaging(world);
}

#[then("a packaging error is returned")]
fn then_packaging_error(world: &mut PackagingWorld) {
    assert!(
        world.packaging_error.is_some(),
        "expected a packaging error"
    );
    assert!(
        matches!(
            world.packaging_error.as_ref().expect("checked above"),
            PackagingError::EmptyFileList
        ),
        "expected EmptyFileList error"
    );
}

#[given("library files \"{a}\" and \"{b}\" and \"{c}\"")]
fn given_three_library_files(world: &mut PackagingWorld, a: String, b: String, c: String) {
    for name in [a, b, c] {
        let path = temp_path(world).join(&name);
        fs::write(&path, format!("content of {name}")).expect("write");
        world.library_files.push(path);
    }
}

#[given("library files \"{a}\" and \"{b}\"")]
fn given_two_library_files(world: &mut PackagingWorld, a: String, b: String) {
    for name in [a, b] {
        let path = temp_path(world).join(&name);
        fs::write(&path, format!("content of {name}")).expect("write");
        world.library_files.push(path);
    }
}

#[then("the archive contains {count} library files")]
fn then_archive_has_n_libraries(world: &mut PackagingWorld, count: usize) {
    let entries = list_archive_entries(world);
    let lib_count = entries.iter().filter(|e| *e != "manifest.json").count();
    assert_eq!(
        lib_count, count,
        "expected {count} library files, got {lib_count}"
    );
}

#[then("the manifest files field contains \"{name}\"")]
fn then_manifest_files_contains(world: &mut PackagingWorld, name: String) {
    let json = world.manifest_json.as_ref().expect("manifest_json set");
    let files = json["files"].as_array().expect("files is an array");
    let names: Vec<&str> = files.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        names.contains(&name.as_str()),
        "files field missing {name}: {names:?}"
    );
}

#[given("a packaged artefact with known components")]
fn given_packaged_with_known(world: &mut PackagingWorld) {
    given_packaged_artefact(world);
}

#[when("the archive filename is inspected")]
#[expect(unused_variables, reason = "rstest-bdd requires the world parameter")]
fn when_filename_inspected(world: &mut PackagingWorld) {
    // Filename is available via output; no additional action needed.
}

#[then("it matches the ArtefactName string representation")]
fn then_filename_matches_artefact_name(world: &mut PackagingWorld) {
    let output = world.output.as_ref().expect("output set");
    let expected = ArtefactName::new(
        world.git_sha.clone().expect("sha"),
        world.toolchain.clone().expect("toolchain"),
        world.target.clone().expect("target"),
    );
    assert_eq!(
        output
            .archive_path
            .file_name()
            .expect("filename")
            .to_string_lossy(),
        expected.filename()
    );
}

/// Extract entry names from a `.tar.zst` archive.
fn list_archive_entries(world: &PackagingWorld) -> Vec<String> {
    let output = world.output.as_ref().expect("output set");
    let file = fs::File::open(&output.archive_path).expect("open");
    let decoder = zstd::Decoder::new(file).expect("decode");
    let mut archive = tar::Archive::new(decoder);
    archive
        .entries()
        .expect("entries")
        .map(|e| {
            let entry = e.expect("entry");
            entry.path().expect("path").to_string_lossy().into_owned()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Package a single library file into a tar.zst archive"
)]
fn scenario_package_single_library(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Manifest JSON contains all required fields"
)]
fn scenario_manifest_fields(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive SHA-256 is a valid digest"
)]
fn scenario_archive_sha256(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Packaging rejects an empty file list"
)]
fn scenario_reject_empty_files(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive filename matches ArtefactName convention"
)]
fn scenario_filename_matches(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive contains multiple library files"
)]
fn scenario_multi_library(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Manifest files field lists all library basenames"
)]
fn scenario_manifest_files_field(world: PackagingWorld) {
    let _ = world;
}
