//! Behaviour-driven tests for artefact packaging.
//!
//! These scenarios validate the packaging pipeline defined in the
//! `artefact::packaging` module against ADR-001 rules. Tests use the
//! rstest-bdd v0.5.0 mutable world pattern with fallible steps.

use std::path::PathBuf;

use cap_std::{ambient_authority, fs::Dir};
use rstest::fixture;
use rstest_bdd_macros::{given, then, when};

#[path = "behaviour_artefact_packaging/archive_assertions.rs"]
mod archive_assertions;
#[path = "behaviour_artefact_packaging/scenarios.rs"]
mod scenarios;
use archive_assertions::list_archive_entries;
use tempfile::TempDir;
use whitaker_installer::artefact::{
    git_sha::GitSha,
    manifest::GeneratedAt,
    naming::ArtefactName,
    packaging::{
        PackageOutput, PackageParams, compute_sha256, generate_manifest_json, package_artefact,
    },
    packaging_error::PackagingError,
    target::TargetTriple,
    toolchain_channel::ToolchainChannel,
};

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

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> PackagingWorld {
    PackagingWorld::default()
}

/// Return the temp directory path, creating the directory if needed.
fn temp_path(world: &mut PackagingWorld) -> Result<PathBuf, String> {
    if world.temp_dir.is_none() {
        let dir = TempDir::new().map_err(|e| format!("create temp dir: {e}"))?;
        world.temp_dir = Some(dir);
    }
    world
        .temp_dir
        .as_ref()
        .map(|dir| dir.path().to_path_buf())
        .ok_or_else(|| String::from("temp_dir set"))
}

/// Fetch the packaging output, failing if packaging has not run successfully.
fn output_ref(world: &PackagingWorld) -> Result<&PackageOutput, String> {
    world
        .output
        .as_ref()
        .ok_or_else(|| String::from("packaging output must be set"))
}

/// Run the packaging pipeline and store the result in the world.
fn run_packaging(world: &mut PackagingWorld) -> Result<(), String> {
    let temp_dir = temp_path(world)?;
    let output_dir = temp_dir.join("dist");
    Dir::open_ambient_dir(&temp_dir, ambient_authority())
        .and_then(|directory| directory.create_dir_all("dist"))
        .map_err(|error| format!("mkdir dist: {error}"))?;

    let params = PackageParams {
        git_sha: world
            .git_sha
            .clone()
            .ok_or_else(|| String::from("git_sha set"))?,
        toolchain: world
            .toolchain
            .clone()
            .ok_or_else(|| String::from("toolchain set"))?,
        target: world
            .target
            .clone()
            .ok_or_else(|| String::from("target set"))?,
        library_files: world.library_files.clone(),
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T00:00:00Z"),
    };

    match package_artefact(params) {
        Ok(output) => world.output = Some(output),
        Err(e) => world.packaging_error = Some(e),
    }
    Ok(())
}

/// Write a fixture library file into the temp directory and register it.
fn add_library_file(world: &mut PackagingWorld, name: &str, content: &[u8]) -> Result<(), String> {
    let temp_dir = temp_path(world)?;
    let path = temp_dir.join(name);
    Dir::open_ambient_dir(&temp_dir, ambient_authority())
        .and_then(|directory| directory.write(name, content))
        .map_err(|error| format!("write {name}: {error}"))?;
    world.library_files.push(path);
    Ok(())
}

/// Populate the world with valid known packaging components.
fn set_known_components(world: &mut PackagingWorld) -> Result<(), String> {
    world.git_sha = Some(GitSha::try_from("abc1234").map_err(|e| format!("valid sha: {e}"))?);
    world.toolchain = Some(
        ToolchainChannel::try_from("nightly-2026-05-28")
            .map_err(|e| format!("valid channel: {e}"))?,
    );
    world.target = Some(
        TargetTriple::try_from("x86_64-unknown-linux-gnu")
            .map_err(|e| format!("valid target: {e}"))?,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("a library file \"{name}\"")]
fn given_library_file(world: &mut PackagingWorld, name: String) -> Result<(), String> {
    add_library_file(world, &name, b"fake library content")
}

#[given("a git SHA \"{sha}\"")]
fn given_git_sha(world: &mut PackagingWorld, sha: String) -> Result<(), String> {
    world.git_sha = Some(GitSha::try_from(sha).map_err(|e| format!("valid SHA: {e}"))?);
    Ok(())
}

#[given("a toolchain channel \"{channel}\"")]
fn given_toolchain(world: &mut PackagingWorld, channel: String) -> Result<(), String> {
    world.toolchain =
        Some(ToolchainChannel::try_from(channel).map_err(|e| format!("valid channel: {e}"))?);
    Ok(())
}

#[given("a target triple \"{triple}\"")]
fn given_target(world: &mut PackagingWorld, triple: String) -> Result<(), String> {
    world.target = Some(TargetTriple::try_from(triple).map_err(|e| format!("valid target: {e}"))?);
    Ok(())
}

#[when("the artefact is packaged")]
fn when_packaged(world: &mut PackagingWorld) -> Result<(), String> {
    run_packaging(world)
}

#[then("the archive exists with the expected ADR-001 filename")]
fn then_archive_exists(world: &mut PackagingWorld) -> Result<(), String> {
    let output = output_ref(world)?;
    if !output.archive_path.exists() {
        return Err(String::from("archive file must exist"));
    }
    let filename = output
        .archive_path
        .file_name()
        .ok_or_else(|| String::from("archive path must have a filename"))?
        .to_string_lossy();
    if !filename.starts_with("whitaker-lints-") {
        return Err(String::from("filename must start with 'whitaker-lints-'"));
    }
    if !filename.ends_with(".tar.zst") {
        return Err(String::from("filename must end with '.tar.zst'"));
    }
    Ok(())
}

#[then("the archive contains the library file")]
fn then_archive_has_library(world: &mut PackagingWorld) -> Result<(), String> {
    let entries = list_archive_entries(world)?;
    let expected: Vec<String> = world
        .library_files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();
    for name in &expected {
        if !entries.contains(name) {
            return Err(format!("archive must contain {name}, got {entries:?}"));
        }
    }
    Ok(())
}

#[then("the archive does not contain a manifest")]
fn then_archive_has_no_manifest(world: &mut PackagingWorld) -> Result<(), String> {
    let entries = list_archive_entries(world)?;
    if entries.contains(&"manifest.json".to_owned()) {
        return Err(String::from("archive must not contain manifest.json"));
    }
    Ok(())
}

#[given("a packaged artefact")]
fn given_packaged_artefact(world: &mut PackagingWorld) -> Result<(), String> {
    add_library_file(world, "libwhitaker_suite.so", b"fake library")?;
    set_known_components(world)?;
    run_packaging(world)
}

#[when("the manifest JSON is generated")]
fn when_manifest_json_generated(world: &mut PackagingWorld) -> Result<(), String> {
    let output = output_ref(world)?;
    let json =
        generate_manifest_json(&output.manifest).map_err(|e| format!("serialization: {e}"))?;
    world.manifest_json =
        Some(serde_json::from_str(&json).map_err(|e| format!("parse JSON: {e}"))?);
    Ok(())
}

#[then("the manifest contains field \"{field}\"")]
fn then_manifest_has_field(world: &mut PackagingWorld, field: String) -> Result<(), String> {
    let json = world
        .manifest_json
        .as_ref()
        .ok_or_else(|| String::from("manifest_json set"))?;
    let obj = json
        .as_object()
        .ok_or_else(|| String::from("manifest JSON must be a top-level object"))?;
    if !obj.contains_key(&field) {
        return Err(format!("missing field: {field}"));
    }
    Ok(())
}

#[when("the archive SHA-256 is computed")]
fn when_sha256_computed(world: &mut PackagingWorld) -> Result<(), String> {
    let output = output_ref(world)?;
    let digest = compute_sha256(&output.archive_path).map_err(|e| format!("sha256: {e}"))?;
    world.archive_sha256 = Some(digest.as_str().to_owned());
    Ok(())
}

#[then("it matches the manifest sha256")]
fn then_digest_matches_manifest(world: &mut PackagingWorld) -> Result<(), String> {
    let archive_hex = world
        .archive_sha256
        .as_ref()
        .ok_or_else(|| String::from("sha256 set"))?;
    let manifest_hex = output_ref(world)?.manifest.sha256().as_str();
    if archive_hex != manifest_hex {
        return Err(format!(
            "archive digest {archive_hex} must match manifest sha256 {manifest_hex}"
        ));
    }
    Ok(())
}

#[then("it is a valid 64-character hex string")]
fn then_valid_hex(world: &mut PackagingWorld) -> Result<(), String> {
    let hex = world
        .archive_sha256
        .as_ref()
        .ok_or_else(|| String::from("sha256 set"))?;
    if hex.len() != 64 {
        return Err(format!("digest must be 64 characters, got {}", hex.len()));
    }
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("digest must be hex: {hex}"));
    }
    Ok(())
}

#[given("no library files")]
fn given_no_files(world: &mut PackagingWorld) -> Result<(), String> {
    world.library_files.clear();
    set_known_components(world)
}

#[when("packaging is attempted")]
fn when_packaging_attempted(world: &mut PackagingWorld) -> Result<(), String> {
    run_packaging(world)
}

#[then("a packaging error is returned")]
fn then_packaging_error(world: &mut PackagingWorld) -> Result<(), String> {
    let error = world
        .packaging_error
        .as_ref()
        .ok_or_else(|| String::from("expected a packaging error"))?;
    if matches!(error, PackagingError::EmptyFileList) {
        Ok(())
    } else {
        Err(format!("expected EmptyFileList error, got {error:?}"))
    }
}

#[given("library files \"{a}\" and \"{b}\" and \"{c}\"")]
fn given_three_library_files(
    world: &mut PackagingWorld,
    a: String,
    b: String,
    c: String,
) -> Result<(), String> {
    for name in [a, b, c] {
        add_library_file(world, &name, format!("content of {name}").as_bytes())?;
    }
    Ok(())
}

#[given("library files \"{a}\" and \"{b}\"")]
fn given_two_library_files(world: &mut PackagingWorld, a: String, b: String) -> Result<(), String> {
    for name in [a, b] {
        add_library_file(world, &name, format!("content of {name}").as_bytes())?;
    }
    Ok(())
}

#[then("the archive contains {count} library files")]
fn then_archive_has_n_libraries(world: &mut PackagingWorld, count: usize) -> Result<(), String> {
    let entries = list_archive_entries(world)?;
    let lib_count = entries.iter().filter(|e| *e != "manifest.json").count();
    if lib_count != count {
        return Err(format!("expected {count} library files, got {lib_count}"));
    }
    Ok(())
}

#[then("the manifest files field contains \"{name}\"")]
fn then_manifest_files_contains(world: &mut PackagingWorld, name: String) -> Result<(), String> {
    let json = world
        .manifest_json
        .as_ref()
        .ok_or_else(|| String::from("manifest_json set"))?;
    let files = json
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| String::from("files must be an array"))?;
    let names: Vec<&str> = files.iter().filter_map(|v| v.as_str()).collect();
    if !names.contains(&name.as_str()) {
        return Err(format!("files field missing {name}: {names:?}"));
    }
    Ok(())
}

#[given("a packaged artefact with known components")]
fn given_packaged_with_known(world: &mut PackagingWorld) -> Result<(), String> {
    given_packaged_artefact(world)
}

#[then("it matches the ArtefactName string representation")]
fn then_filename_matches_artefact_name(world: &mut PackagingWorld) -> Result<(), String> {
    let expected = ArtefactName::new(
        world
            .git_sha
            .clone()
            .ok_or_else(|| String::from("sha set"))?,
        world
            .toolchain
            .clone()
            .ok_or_else(|| String::from("toolchain set"))?,
        world
            .target
            .clone()
            .ok_or_else(|| String::from("target set"))?,
    );
    let output = output_ref(world)?;
    let filename = output
        .archive_path
        .file_name()
        .ok_or_else(|| String::from("archive path must have a filename"))?
        .to_string_lossy();
    if filename != expected.filename() {
        return Err(format!(
            "filename {filename} must match ArtefactName {}",
            expected.filename()
        ));
    }
    Ok(())
}
