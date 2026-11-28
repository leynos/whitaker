//! Builds and stages the lint library for the generated emission tests.
//!
//! Building is slow and must happen once per test process, so the staged path
//! is cached in a [`OnceLock`]. The staged copy uses a private directory to
//! avoid racing the UI harness's toolchain-qualified library.

use std::{
    env,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use anyhow::Context as _;
use cap_std::{ambient_authority, fs::Dir};
use cargo_metadata::{Message, Metadata, MetadataCommand};

/// The lint crate under test.
pub(super) const LINT_CRATE_NAME: &str = "string_continuation_style";

/// Returns the staged lint library, building it once per test process.
pub(super) fn lint_library_path() -> anyhow::Result<&'static Path> {
    static LIBRARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let result = LIBRARY.get_or_init(|| build_lint_library().map_err(|error| error.to_string()));

    match result {
        Ok(path) => Ok(path.as_path()),
        Err(message) => Err(anyhow::anyhow!("{message}")),
    }
}

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
    Dir::create_ambient_dir_all(&directory, ambient_authority())
        .context("failed to create the staged lint directory")?;

    let file_name = cdylib_path
        .file_name()
        .context("the lint cdylib should have a filename")?
        .to_string_lossy();
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{}", pinned_toolchain()),
        |stripped| format!("{stripped}@{}{suffix}", pinned_toolchain()),
    );

    let source_directory = cdylib_path
        .parent()
        .context("the lint cdylib should have a parent directory")?;
    let source = Dir::open_ambient_dir(source_directory, ambient_authority())
        .context("failed to open the lint build directory")?;
    let bytes = source
        .read(file_name.as_ref())
        .context("failed to read the built lint library")?;
    let target = Dir::open_ambient_dir(&directory, ambient_authority())
        .context("failed to open the staged lint directory")?;
    target
        .write(target_name.as_str(), &bytes)
        .with_context(|| format!("failed to copy lint library to {target_name}"))?;

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
pub(super) fn pinned_toolchain() -> String {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned())
}
