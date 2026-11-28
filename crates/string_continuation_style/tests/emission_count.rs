//! Generated emission-count coverage for the early lint pass.
//!
//! The classifier property proves one `PreferConcat` disposition per literal,
//! but it never runs the driver. This test generates a fixture crate with
//! literals carrying one through ten joins, runs the real lint through
//! `cargo dylint`, and counts the emitted diagnostics. A per-join emitter would
//! produce 55 warnings; a suppressed emission would produce fewer than ten.

use std::{
    env,
    fmt::Write as _,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context as _;
use cargo_metadata::{Message, Metadata, MetadataCommand};
use serial_test::serial;
use tempfile::{Builder, TempDir};

const LINT_CRATE_NAME: &str = "string_continuation_style";
const MAX_JOINS: usize = 10;

/// Builds the lint library and stages it under a private toolchain-qualified
/// name.
///
/// The private directory keeps the copy away from the UI harness's own
/// toolchain-qualified library, so the two test binaries cannot race on the
/// same file.
fn build_lint_library() -> anyhow::Result<PathBuf> {
    let metadata = MetadataCommand::new()
        .no_deps()
        .other_options(vec!["--frozen".to_owned()])
        .exec()
        .context("failed to fetch cargo metadata")?;

    let output = run_lint_crate_build(metadata.workspace_root.as_std_path())?;
    let package_id = find_package_id(&metadata, LINT_CRATE_NAME)?;
    let cdylib_path = find_cdylib_in_artefacts(&output, &package_id)?;

    let directory = metadata
        .workspace_root
        .as_std_path()
        .join("target/emission-count")
        .join(pinned_toolchain());
    fs::create_dir_all(&directory).context("failed to create the staged lint directory")?;

    let file_name = cdylib_path
        .file_name()
        .context("the lint cdylib should have a filename")?
        .to_string_lossy();
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{}", pinned_toolchain()),
        |stripped| format!("{stripped}@{}{suffix}", pinned_toolchain()),
    );
    let target_path = directory.join(target_name);
    fs::copy(&cdylib_path, &target_path)
        .with_context(|| format!("failed to copy lint library to {}", target_path.display()))?;

    Ok(directory)
}

/// Executes `cargo build` for the lint crate and returns its JSON output.
fn run_lint_crate_build(workspace_root: &Path) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--frozen")
        .arg("--lib")
        .arg("--quiet")
        .arg("--message-format=json")
        .arg("--package")
        .arg(LINT_CRATE_NAME)
        .arg("--features")
        .arg("dylint-driver")
        .current_dir(workspace_root)
        .output()
        .context("failed to execute the lint library build")?;

    if !output.status.success() {
        anyhow::bail!(
            "lint library build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

/// Returns the workspace package id for `crate_name`.
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
        .with_context(|| format!("lint crate `{crate_name}` not found in the workspace"))
}

/// Extracts the lint cdylib path from the build's JSON messages.
fn find_cdylib_in_artefacts(
    stdout: &[u8],
    package_id: &cargo_metadata::PackageId,
) -> anyhow::Result<PathBuf> {
    for parsed in Message::parse_stream(Cursor::new(stdout)) {
        let message = parsed.context("failed to parse the cargo build output")?;
        let Message::CompilerArtifact(artefact) = message else {
            continue;
        };
        if artefact.package_id != *package_id || !artefact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artefact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Ok(path.clone().into_std_path_buf());
        }
    }

    anyhow::bail!("cdylib artefact not found for `{package_id}`")
}

/// Returns the toolchain the lint library is built with.
fn pinned_toolchain() -> String {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned())
}

/// Returns the workspace target directory used for generated fixtures.
///
/// Building inside the workspace target keeps the fixture's Dylint target
/// directory off `/tmp`, which is scratch space rather than a build target.
fn fixture_root() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
            PathBuf::from,
        )
        .join("emission-count-fixtures")
}

/// Writes a fixture crate whose literals have one through ten joins.
fn create_fixture() -> anyhow::Result<TempDir> {
    let root = fixture_root();
    fs::create_dir_all(&root).context("failed to create the emission fixture root")?;
    let fixture = Builder::new()
        .prefix("emission-count-")
        .tempdir_in(&root)
        .context("failed to create the emission fixture directory")?;
    fs::write(
        fixture.path().join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"emission-count-fixture\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[workspace]\n",
        ),
    )
    .context("failed to write the fixture manifest")?;
    fs::create_dir(fixture.path().join("src"))
        .context("failed to create the fixture source directory")?;
    fs::write(fixture.path().join("src/lib.rs"), generated_source()?)
        .context("failed to write the fixture source")?;
    Ok(fixture)
}

/// Builds a source file with one eligible literal per join count.
fn generated_source() -> anyhow::Result<String> {
    let mut source = String::from("//! Generated emission fixture.\n#![allow(dead_code)]\n\n");
    for joins in 1..=MAX_JOINS {
        let fragments = (0..=joins)
            .map(|index| format!("f{joins}_{index}"))
            .collect::<Vec<_>>();
        writeln!(
            source,
            "const L{joins}: &str = \"{}\";",
            fragments.join(" \\\n    ")
        )?;
    }
    Ok(source)
}

/// The result of running `cargo dylint` against the fixture.
struct CargoDylintResult {
    is_success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Runs `cargo dylint` for the named lint against `fixture`.
fn run_cargo_dylint(fixture: &Path, library_path: &Path) -> anyhow::Result<CargoDylintResult> {
    let output = Command::new("cargo")
        .arg("dylint")
        .arg("--lib")
        .arg(LINT_CRATE_NAME)
        .arg("--")
        .arg("--message-format")
        .arg("json")
        .current_dir(fixture)
        .env("DYLINT_LIBRARY_PATH", library_path)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env("CARGO_NET_OFFLINE", "true")
        .env_remove("RUSTFLAGS")
        .env_remove("DYLINT_RUSTFLAGS")
        .output()
        .context("failed to execute cargo dylint")?;

    Ok(CargoDylintResult {
        is_success: output.status.success(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Counts the fixture's emitted `string_continuation_style` diagnostics.
fn diagnostic_count(output: &[u8]) -> anyhow::Result<usize> {
    let messages = Message::parse_stream(Cursor::new(output))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to parse the cargo JSON output")?;

    Ok(messages
        .into_iter()
        .filter_map(|message| match message {
            Message::CompilerMessage(compiler_message) => Some(compiler_message.message),
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

#[test]
#[serial]
fn generated_literals_emit_exactly_one_diagnostic_each() -> anyhow::Result<()> {
    let library_path = build_lint_library()?;
    let fixture = create_fixture()?;
    let result = run_cargo_dylint(fixture.path(), &library_path)?;

    anyhow::ensure!(
        result.is_success,
        "cargo dylint failed for the generated fixture: {}",
        result.stderr
    );
    let count = diagnostic_count(&result.stdout)?;
    anyhow::ensure!(
        count == MAX_JOINS,
        "expected {MAX_JOINS} diagnostics for {} generated literals, got {count}\nstderr:\n{}",
        MAX_JOINS,
        result.stderr
    );

    Ok(())
}
