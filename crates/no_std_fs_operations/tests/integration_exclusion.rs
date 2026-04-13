//! Integration tests for the `no_std_fs_operations` lint exclusion feature.
//!
//! These tests verify that the `excluded_crates` configuration correctly suppresses
//! diagnostics for specified crates. The tests invoke `cargo dylint` as a subprocess
//! against fixture projects with different configurations.
//!
//! # Prerequisites
//!
//! - `cargo-dylint` and `dylint-link` must be installed
//! - The workspace must be buildable so the harness can build the lint library
//!
//! These tests are marked `#[ignore]` by default because they require external
//! dependencies. Run with `--ignored` to execute.

use std::env;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use cargo_metadata::{Message, Metadata, MetadataCommand};
use rstest::{fixture, rstest};
use serial_test::serial;

const LINT_CRATE_NAME: &str = "no_std_fs_operations";

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

/// Result of invoking `cargo dylint` against a fixture crate.
struct CargoDylintResult {
    is_success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Runs `cargo dylint` on the given fixture project directory.
fn run_cargo_dylint(fixture_dir: &Path, library_path: &Path) -> CargoDylintResult {
    let output = Command::new("cargo")
        .arg("dylint")
        .arg("--all")
        .arg("--")
        .arg("--message-format")
        .arg("json")
        .current_dir(fixture_dir)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .env("DYLINT_RUSTFLAGS", "-D warnings")
        .output()
        .expect("failed to execute cargo dylint");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    CargoDylintResult {
        is_success: output.status.success(),
        stdout: output.stdout,
        stderr,
    }
}

/// Counts diagnostics emitted by the `no_std_fs_operations` lint from cargo JSON output.
fn diagnostic_count(output: &[u8]) -> usize {
    Message::parse_stream(Cursor::new(output))
        .map(|message| message.expect("cargo dylint should emit valid JSON messages"))
        .filter_map(|message| match message {
            Message::CompilerMessage(message) => Some(message.message),
            _ => None,
        })
        .filter(|diagnostic| {
            diagnostic
                .code
                .as_ref()
                .is_some_and(|code| code.code == LINT_CRATE_NAME)
        })
        .count()
}

/// Returns the path to a named fixture project under `tests/fixtures/`.
fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[fixture]
fn lint_library_path() -> PathBuf {
    static LINT_LIBRARY_PATH: OnceLock<PathBuf> = OnceLock::new();

    LINT_LIBRARY_PATH.get_or_init(build_lint_library).clone()
}

#[rstest]
#[case("excluded_project", false, true)]
#[case("non_excluded_project", true, false)]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn exclusion_behaviour_matches_fixture_configuration(
    lint_library_path: PathBuf,
    #[case] fixture: &str,
    #[case] expect_diagnostics: bool,
    #[case] expected_success: bool,
) {
    let fixture_dir = fixture_path(fixture);

    let result = run_cargo_dylint(&fixture_dir, &lint_library_path);
    let count = diagnostic_count(&result.stdout);

    assert!(
        result.is_success == expected_success,
        "fixture `{fixture}` should return success={expected_success}, but stderr was:\n{}",
        result.stderr
    );

    if expect_diagnostics {
        assert!(
            count > 0,
            "fixture `{fixture}` should emit `no_std_fs_operations` diagnostics, but stderr was:\n{}",
            result.stderr
        );
    } else {
        assert!(
            count == 0,
            "fixture `{fixture}` should emit zero `no_std_fs_operations` diagnostics, but stderr was:\n{}",
            result.stderr
        );
    }
}
