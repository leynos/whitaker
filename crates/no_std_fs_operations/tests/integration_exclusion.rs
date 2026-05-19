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

use anyhow::Context as _;
use cargo_metadata::{Message, Metadata, MetadataCommand};
use insta::assert_json_snapshot;
use rstest::rstest;
use serial_test::serial;

mod test_support;

use test_support::create_fixture_project;

const LINT_CRATE_NAME: &str = "no_std_fs_operations";

/// Builds the lint library and returns the path to the release directory.
fn build_lint_library() -> anyhow::Result<PathBuf> {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to fetch cargo metadata")?;

    let output = run_lint_crate_build(metadata.workspace_root.as_std_path())?;
    let package_id = find_package_id(&metadata, LINT_CRATE_NAME)?;
    let cdylib_path = find_cdylib_in_artifacts(&output, &package_id)?;

    let release_dir = cdylib_path
        .parent()
        .context("cdylib should have a parent directory")?
        .to_path_buf();

    stage_toolchain_qualified_library(&cdylib_path, &release_dir)?;

    Ok(release_dir)
}

/// Executes `cargo build` for the lint crate and returns the build output.
fn run_lint_crate_build(workspace_root: &Path) -> anyhow::Result<Vec<u8>> {
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
        .context("failed to execute cargo build")?;

    if !output.status.success() {
        anyhow::bail!(
            "lint library build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

/// Copies the built library to a toolchain-qualified filename for Dylint discovery.
fn stage_toolchain_qualified_library(cdylib_path: &Path, release_dir: &Path) -> anyhow::Result<()> {
    let toolchain = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned());

    let file_name = cdylib_path
        .file_name()
        .context("cdylib should have a filename")?
        .to_string_lossy();

    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );

    let target_path = release_dir.join(&target_name);
    std::fs::copy(cdylib_path, &target_path)
        .with_context(|| format!("failed to copy lint library to {}", target_path.display()))?;
    Ok(())
}

/// Locates the package ID for a workspace member by crate name.
fn find_package_id(
    metadata: &Metadata,
    crate_name: &str,
) -> anyhow::Result<cargo_metadata::PackageId> {
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
        .with_context(|| format!("lint crate `{crate_name}` not found in workspace"))
}

/// Extracts the cdylib path from cargo build JSON output for a given package.
fn find_cdylib_in_artifacts(
    stdout: &[u8],
    package_id: &cargo_metadata::PackageId,
) -> anyhow::Result<PathBuf> {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let message = message.context("failed to parse cargo build JSON output")?;
        let Message::CompilerArtifact(artifact) = message else {
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
            return Ok(path.clone().into_std_path_buf());
        }
    }

    anyhow::bail!("cdylib artifact not found in build output for package `{package_id}`")
}

/// Result of invoking `cargo dylint` against a fixture crate.
struct CargoDylintResult {
    is_success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Runs `cargo dylint` on the given fixture project directory.
fn run_cargo_dylint(fixture_dir: &Path, library_path: &Path) -> anyhow::Result<CargoDylintResult> {
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
        .context("failed to execute cargo dylint")?;

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    Ok(CargoDylintResult {
        is_success: output.status.success(),
        stdout: output.stdout,
        stderr,
    })
}

/// Counts diagnostics emitted by the `no_std_fs_operations` lint from cargo JSON output.
///
/// Propagates parse errors rather than panicking, including context from the raw stdout
/// so callers can inspect malformed cargo output.
fn diagnostic_count(output: &[u8]) -> Result<usize, anyhow::Error> {
    let messages: Vec<Message> = Message::parse_stream(Cursor::new(output))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to parse cargo JSON messages: {}\nstdout:\n{}",
                e,
                String::from_utf8_lossy(output)
            )
        })?;

    Ok(messages
        .into_iter()
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
        .count())
}

/// Replaces all occurrences of `prefix` in `value` with the fixed
/// placeholder `[FIXTURE_ROOT]` so snapshot output is stable across runs.
fn redact_path_prefix(value: serde_json::Value, prefix: &str) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(s.replace(prefix, "[FIXTURE_ROOT]"))
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.into_iter()
                .map(|value| redact_path_prefix(value, prefix))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, redact_path_prefix(value, prefix)))
                .collect(),
        ),
        other => other,
    }
}

#[expect(
    clippy::useless_asref,
    clippy::redundant_closure,
    reason = "anyhow::Error is not Clone, so .as_ref().map(Clone::clone) is necessary to convert &Result<PathBuf, Error> into Result<PathBuf, Error>"
)]
fn lint_library_path() -> anyhow::Result<PathBuf> {
    static LINT_LIBRARY_PATH: OnceLock<anyhow::Result<PathBuf>> = OnceLock::new();

    LINT_LIBRARY_PATH
        .get_or_init(|| build_lint_library())
        .as_ref()
        .map(Clone::clone)
        .map_err(|e| anyhow::anyhow!("{e:#}"))
}

struct Expectation {
    should_emit_diagnostics: bool,
    should_succeed: bool,
}

/// Shared driver for exclusion integration tests.
fn run_exclusion_test(
    crate_name: &str,
    is_excluded: bool,
    expectation: Expectation,
) -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_fixture_project(crate_name, is_excluded)
        .context("failed to create fixture project")?;
    assert_fixture_behaviour(fixture.root(), &lint_library_path, crate_name, expectation)
}

#[rstest]
#[case(
    "excluded_test_crate",
    true,
    Expectation {
        should_emit_diagnostics: false,
        should_succeed: true,
    }
)]
#[case(
    "non_excluded_crate",
    false,
    Expectation {
        should_emit_diagnostics: true,
        should_succeed: false,
    }
)]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn exclusion_crates_behaviour_test(
    #[case] crate_name: &str,
    #[case] is_excluded: bool,
    #[case] expected: Expectation,
) -> anyhow::Result<()> {
    run_exclusion_test(crate_name, is_excluded, expected)
}

/// Runs `cargo dylint` against the fixture and counts diagnostics.
fn evaluate_fixture(
    fixture_dir: &Path,
    lint_library_path: &Path,
    crate_name: &str,
) -> anyhow::Result<(bool, usize)> {
    let result = run_cargo_dylint(fixture_dir, lint_library_path)?;
    let count = diagnostic_count(&result.stdout).with_context(|| {
        format!(
            "crate `{crate_name}` produced malformed cargo output\nstderr:\n{}",
            result.stderr
        )
    })?;
    Ok((result.is_success, count))
}

fn assert_fixture_behaviour(
    fixture_dir: &Path,
    lint_library_path: &Path,
    crate_name: &str,
    expectation: Expectation,
) -> anyhow::Result<()> {
    let (is_success, count) = evaluate_fixture(fixture_dir, lint_library_path, crate_name)?;

    assert!(
        is_success == expectation.should_succeed,
        "crate `{crate_name}` should return success={}",
        expectation.should_succeed
    );

    if expectation.should_emit_diagnostics {
        assert!(
            count > 0,
            "crate `{crate_name}` should emit `no_std_fs_operations` diagnostics"
        );
    } else {
        assert!(
            count == 0,
            "crate `{crate_name}` should emit zero `no_std_fs_operations` diagnostics"
        );
    }

    Ok(())
}

/// Snapshot test: verifies the structured JSON diagnostic output emitted by
/// `cargo dylint` for a non-excluded crate.
///
/// Non-deterministic fields (absolute fixture paths) are redacted to
/// `[FIXTURE_ROOT]` before the snapshot is taken.
#[test]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn non_excluded_crate_diagnostics_match_snapshot() -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_fixture_project("non_excluded_crate_snap", false)
        .context("failed to create fixture project")?;

    let result = run_cargo_dylint(fixture.root(), &lint_library_path)
        .context("failed to run cargo dylint")?;

    let diagnostics: Vec<serde_json::Value> =
        Message::parse_stream(Cursor::new(&result.stdout))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| {
                panic!(
                    "non_excluded_crate_snap produced malformed cargo output: {e}\nstderr:\n{}",
                    result.stderr
                )
            })
            .into_iter()
            .filter_map(|message| match message {
                Message::CompilerMessage(message)
                    if message
                        .message
                        .code
                        .as_ref()
                        .is_some_and(|code| code.code == LINT_CRATE_NAME) =>
                {
                    Some(serde_json::to_value(message.message).unwrap_or_else(|e| {
                        panic!("failed to serialise diagnostic for snapshot: {e}")
                    }))
                }
                _ => None,
            })
            .collect();

    let prefix = fixture
        .root()
        .to_str()
        .context("fixture root should be valid UTF-8")?;

    let redacted: Vec<serde_json::Value> = diagnostics
        .into_iter()
        .map(|value| redact_path_prefix(value, prefix))
        .collect();

    assert_json_snapshot!("non_excluded_crate_diagnostics", redacted);
    Ok(())
}
