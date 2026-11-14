//! Nested Cargo invocation and toolchain discovery for the parity runs.
//!
//! The parity check builds this lint crate and then drives both pinned Clippy
//! and Dylint over a generated fixture. Those runs need an isolated target
//! directory, a library staged under a toolchain-qualified file name, and the
//! toolchain that Dylint itself will look for.

use std::{env, io::Cursor, process::Command};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cargo_metadata::Message;
use tempfile::TempDir;

/// The lint package under test.
pub(crate) const LINT_PACKAGE: &str = "macro_aware_cognitive_complexity";

/// Run `args` through Cargo, advising installation when the tool is missing.
pub(crate) fn ensure_tool(description: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(args)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .output()
        .with_context(|| format!("failed to run `{description}`; is it installed?"))?;
    if !output.status.success() {
        anyhow::bail!(
            "`{description}` failed; install Clippy with `rustup component add clippy` and Dylint \
             with `cargo install cargo-dylint dylint-link`; stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Find the workspace root containing this crate's manifest.
pub(crate) fn workspace_root() -> anyhow::Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidate = manifest_dir.as_path();
    loop {
        if is_workspace_root(candidate)? {
            return Ok(candidate.to_path_buf());
        }
        candidate = candidate
            .parent()
            .context("workspace root not found above CARGO_MANIFEST_DIR")?;
    }
}

/// Check whether `candidate` contains a workspace manifest.
///
/// The result is `false` only when the manifest is absent or not a workspace;
/// an unreadable directory or manifest is an error.
pub(crate) fn is_workspace_root(candidate: &Utf8Path) -> anyhow::Result<bool> {
    let directory = Dir::open_ambient_dir(candidate, ambient_authority())
        .with_context(|| format!("failed to open candidate directory `{candidate}`"))?;
    match directory.read_to_string("Cargo.toml") {
        Ok(manifest) => Ok(manifest.contains("[workspace]")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("failed to read workspace manifest in `{candidate}`")),
    }
}

/// Create an isolated target directory under the workspace's `target/tests`.
///
/// Neither `/tmp` nor the workspace's own `debug` directory is shared with a
/// staged lint library.
pub(crate) fn private_target_dir(
    workspace_root: &Utf8Path,
    label: &str,
) -> anyhow::Result<TempDir> {
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())
        .context("failed to open the workspace for an isolated target directory")?;
    workspace
        .create_dir_all("target/tests/parity")
        .context("failed to create the isolated target parent directory")?;
    TempDir::new_in(workspace_root.join("target/tests/parity").as_std_path())
        .with_context(|| format!("failed to create the isolated `{label}` target directory"))
}

/// Build the lint library and stage a toolchain-qualified copy for Dylint.
pub(crate) fn build_and_stage_library(
    workspace_root: &Utf8Path,
    target_dir: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--lib",
            "--quiet",
            "--message-format=json",
            "-p",
            LINT_PACKAGE,
            "--features",
            "dylint-driver",
        ])
        .current_dir(workspace_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .context("failed to execute cargo build for the lint library")?;
    if !output.status.success() {
        anyhow::bail!(
            "lint library build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let cdylib_path = find_cdylib_in_artefacts(&output.stdout)?;
    let debug_dir = cdylib_path
        .parent()
        .context("staged cdylib has no parent directory")?
        .to_owned();
    stage_toolchain_qualified_library(&cdylib_path, &debug_dir)?;
    Ok(debug_dir)
}

/// Extract the lint package's cdylib path from cargo build JSON output.
pub(crate) fn find_cdylib_in_artefacts(stdout: &[u8]) -> anyhow::Result<Utf8PathBuf> {
    for parsed in Message::parse_stream(Cursor::new(stdout)) {
        let message = parsed.context("failed to parse cargo build JSON output")?;
        let Message::CompilerArtifact(artefact) = message else {
            continue;
        };
        if artefact.target.name != LINT_PACKAGE || !artefact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artefact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Ok(path.clone());
        }
    }
    anyhow::bail!("cdylib artefact for `{LINT_PACKAGE}` not found in build output")
}

/// Copy the cdylib to a toolchain-qualified filename for Dylint discovery.
pub(crate) fn stage_toolchain_qualified_library(
    cdylib_path: &Utf8Path,
    directory: &Utf8Path,
) -> anyhow::Result<()> {
    let toolchain = pinned_toolchain()?;
    let file_name = cdylib_path
        .file_name()
        .context("staged cdylib has no file name")?;
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let staged_directory = Dir::open_ambient_dir(directory, ambient_authority())
        .context("failed to open the staged library directory")?;
    staged_directory
        .copy(file_name, &staged_directory, &target_name)
        .with_context(|| format!("failed to stage lint library as `{target_name}`"))?;
    Ok(())
}

/// Return the toolchain the lint library is built with.
///
/// `cargo dylint` is told the same toolchain explicitly, so the fixture
/// directory's own toolchain resolution cannot pick a different nightly.
///
/// # Errors
///
/// Return an error when neither the run-time nor the compile-time
/// `RUSTUP_TOOLCHAIN` is set, so the pinned toolchain cannot be determined. The
/// test must then run through rustup's Cargo proxy, which sets it from
/// `rust-toolchain.toml`; a sentinel would stage a library under a name Dylint
/// never searches for.
pub(crate) fn pinned_toolchain() -> anyhow::Result<String> {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .context(
            "cannot determine the pinned toolchain: neither the run-time nor the compile-time \
             RUSTUP_TOOLCHAIN is set; run this test through rustup's Cargo proxy so it selects \
             the toolchain from `rust-toolchain.toml`",
        )
}
