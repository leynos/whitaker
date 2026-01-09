//! Integration tests for the `no_std_fs_operations` lint exclusion feature.
//!
//! These tests verify that the `excluded_crates` configuration correctly suppresses
//! diagnostics for specified crates. The tests invoke `cargo dylint` as a subprocess
//! against fixture projects with different configurations.
//!
//! # Prerequisites
//!
//! - `cargo-dylint` and `dylint-link` must be installed
//! - The lint library must be built before running these tests
//!
//! These tests are marked `#[ignore]` by default because they require external
//! dependencies and a built lint library. Run with `--ignored` to execute.

use std::env;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{Message, Metadata, MetadataCommand};

const LINT_CRATE_NAME: &str = "no_std_fs_operations";
const DIAGNOSTIC_MARKER: &str = "std::fs operations bypass";

/// Builds the lint library and returns the path to the release directory.
fn build_lint_library() -> PathBuf {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("failed to fetch cargo metadata");

    let output = run_lint_crate_build(metadata.workspace_root.as_std_path());
    let package_id = find_package_id(&metadata, LINT_CRATE_NAME);
    let cdylib_path = find_cdylib_in_artifacts(&output, &package_id);

    let release_dir = cdylib_path
        .parent()
        .expect("cdylib should have a parent directory")
        .to_path_buf();

    stage_toolchain_qualified_library(&cdylib_path, &release_dir);

    release_dir
}

/// Executes `cargo build` for the lint crate and returns the build output.
fn run_lint_crate_build(workspace_root: &Path) -> Vec<u8> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .arg("--message-format=json")
        .arg("--package")
        .arg(LINT_CRATE_NAME)
        .arg("--features")
        .arg("dylint-driver")
        .current_dir(workspace_root)
        .output()
        .expect("failed to execute cargo build");

    assert!(
        output.status.success(),
        "lint library build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

/// Copies the built library to a toolchain-qualified filename for Dylint discovery.
fn stage_toolchain_qualified_library(cdylib_path: &Path, release_dir: &Path) {
    let toolchain = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned());

    let file_name = cdylib_path
        .file_name()
        .expect("cdylib should have a filename")
        .to_string_lossy();

    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );

    let target_path = release_dir.join(&target_name);
    std::fs::copy(cdylib_path, &target_path).expect("failed to copy lint library");
}

/// Locates the package ID for a workspace member by crate name.
fn find_package_id(metadata: &Metadata, crate_name: &str) -> cargo_metadata::PackageId {
    metadata
        .packages
        .iter()
        .find(|package| {
            package.name == crate_name
                && metadata
                    .workspace_members
                    .iter()
                    .any(|member| member == &package.id)
        })
        .map(|package| package.id.clone())
        .expect("lint crate not found in workspace")
}

/// Extracts the cdylib path from cargo build JSON output for a given package.
fn find_cdylib_in_artifacts(stdout: &[u8], package_id: &cargo_metadata::PackageId) -> PathBuf {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let Ok(Message::CompilerArtifact(artifact)) = message else {
            continue;
        };

        if artifact.package_id != *package_id {
            continue;
        }

        if !artifact.target.is_cdylib() {
            continue;
        }

        if let Some(path) = artifact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return path.clone().into_std_path_buf();
        }
    }

    panic!("cdylib artifact not found in build output");
}

/// Runs `cargo dylint` on the given fixture project directory.
fn run_cargo_dylint(fixture_dir: &Path, library_path: &Path) -> (bool, String, String) {
    let output = Command::new("cargo")
        .arg("dylint")
        .arg("--all")
        .arg("--")
        .arg("-D")
        .arg("warnings")
        .current_dir(fixture_dir)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .output()
        .expect("failed to execute cargo dylint");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    (output.status.success(), stdout, stderr)
}

/// Returns the path to a named fixture project under `tests/fixtures/`.
fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
#[ignore = "requires cargo-dylint and built lint library"]
fn excluded_crate_suppresses_diagnostics() {
    let library_path = build_lint_library();
    let fixture_dir = fixture_path("excluded_project");

    let (success, _stdout, stderr) = run_cargo_dylint(&fixture_dir, &library_path);

    assert!(
        !stderr.contains(DIAGNOSTIC_MARKER),
        "excluded crate should NOT emit '{}' diagnostic, but stderr was:\n{}",
        DIAGNOSTIC_MARKER,
        stderr
    );

    // The build should succeed because the lint is suppressed
    assert!(
        success,
        "cargo dylint should succeed for excluded crate, stderr:\n{}",
        stderr
    );
}

#[test]
#[ignore = "requires cargo-dylint and built lint library"]
fn non_excluded_crate_emits_diagnostics() {
    let library_path = build_lint_library();
    let fixture_dir = fixture_path("non_excluded_project");

    let (_success, _stdout, stderr) = run_cargo_dylint(&fixture_dir, &library_path);

    assert!(
        stderr.contains(DIAGNOSTIC_MARKER),
        "non-excluded crate should emit '{}' diagnostic, but stderr was:\n{}",
        DIAGNOSTIC_MARKER,
        stderr
    );
}
