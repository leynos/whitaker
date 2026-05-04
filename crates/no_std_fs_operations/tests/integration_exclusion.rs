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
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::Context as _;
use cargo_metadata::{Message, Metadata, MetadataCommand};
use rstest::rstest;
use serial_test::serial;
use tempfile::TempDir;

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

/// Standalone project fixture created in a temporary directory for integration tests.
struct FixtureProject {
    _temp_dir: TempDir,
    root: PathBuf,
}

impl FixtureProject {
    fn root(&self) -> &Path {
        &self.root
    }
}

/// Creates a temporary fixture project for verifying exclusion behaviour.
fn create_fixture_project(crate_name: &str, is_excluded: bool) -> anyhow::Result<FixtureProject> {
    let temp_dir = TempDir::new().context("failed to create temporary fixture directory")?;
    let root = temp_dir.path().to_path_buf();

    fs::write(
        root.join("Cargo.toml"),
        format!(
            concat!(
                "[package]\n",
                "name = \"{crate_name}\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2024\"\n",
                "\n",
                "[dependencies]\n",
            ),
            crate_name = crate_name
        ),
    )
    .context("failed to write fixture Cargo.toml")?;

    fs::write(
        root.join("dylint.toml"),
        fixture_dylint_config(crate_name, is_excluded),
    )
    .context("failed to write fixture dylint.toml")?;

    let source_dir = root.join("src");
    fs::create_dir(&source_dir).context("failed to create fixture src directory")?;
    fs::write(source_dir.join("lib.rs"), fixture_source(crate_name))
        .context("failed to write fixture source")?;

    Ok(FixtureProject {
        _temp_dir: temp_dir,
        root,
    })
}

fn fixture_dylint_config(crate_name: &str, is_excluded: bool) -> String {
    let excluded_crates = if is_excluded {
        format!("[\"{crate_name}\"]")
    } else {
        "[]".to_owned()
    };

    format!(
        concat!(
            "[no_std_fs_operations]\n",
            "excluded_crates = {excluded_crates}\n",
        ),
        excluded_crates = excluded_crates
    )
}

fn fixture_source(crate_name: &str) -> String {
    format!(
        concat!(
            "//! Temporary fixture crate for `no_std_fs_operations` integration tests.\n",
            "\n",
            "use std::fs::File;\n",
            "use std::path::Path;\n",
            "\n",
            "/// Opens a file for reading.\n",
            "///\n",
            "/// # Examples\n",
            "///\n",
            "/// ```no_run\n",
            "/// use {crate_name}::open_file;\n",
            "///\n",
            "/// let file = open_file(\"Cargo.toml\").expect(\"file should exist\");\n",
            "/// let result = open_file(\"nonexistent.txt\");\n",
            "/// assert!(result.is_err());\n",
            "/// # drop(file);\n",
            "/// ```\n",
            "pub fn open_file<P: AsRef<Path>>(path: P) -> std::io::Result<File> {{\n",
            "    File::open(path)\n",
            "}}\n",
        ),
        crate_name = crate_name
    )
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

// anyhow::Error is not Clone, so .as_ref().map(Clone::clone) is necessary
// to convert &Result<PathBuf, Error> into Result<PathBuf, Error>.
#[allow(clippy::useless_asref, clippy::redundant_closure)]
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
fn run_exclusion_test(crate_name: &str, is_excluded: bool, expectation: Expectation) {
    let lint_library_path = lint_library_path().expect("failed to build lint library");
    let fixture =
        create_fixture_project(crate_name, is_excluded).expect("failed to create fixture project");
    assert_fixture_behaviour(fixture.root(), &lint_library_path, crate_name, expectation);
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
) {
    run_exclusion_test(crate_name, is_excluded, expected);
}

/// Runs `cargo dylint` against the fixture and counts diagnostics.
///
/// This function is not called directly by `exclusion_crates_behaviour_test` but is exposed
/// as a reusable fallible evaluation primitive for test code that needs to inspect results
/// programmatically.
#[allow(dead_code)]
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
) {
    let result = run_cargo_dylint(fixture_dir, lint_library_path)
        .unwrap_or_else(|e| panic!("crate `{crate_name}`: failed to run cargo dylint: {e:#}"));
    let count = diagnostic_count(&result.stdout).unwrap_or_else(|e| {
        panic!(
            "crate `{crate_name}` produced malformed cargo output: {e:#}\nstderr:\n{}",
            result.stderr
        )
    });

    assert!(
        result.is_success == expectation.should_succeed,
        "crate `{crate_name}` should return success={}, but stderr was:\n{}",
        expectation.should_succeed,
        result.stderr
    );

    if expectation.should_emit_diagnostics {
        assert!(
            count > 0,
            "crate `{crate_name}` should emit `no_std_fs_operations` diagnostics, \
             but stderr was:\n{}",
            result.stderr
        );
    } else {
        assert!(
            count == 0,
            "crate `{crate_name}` should emit zero `no_std_fs_operations` diagnostics, \
             but stderr was:\n{}",
            result.stderr
        );
    }
}
