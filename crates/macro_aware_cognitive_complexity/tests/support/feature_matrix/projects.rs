//! Generated standalone projects and their exact dependency pins.
//!
//! Each project is its own Cargo workspace, so the ordinary build and the
//! generated fixture cannot see each other's dependency graph. Versions are
//! read from the workspace lockfile at run time rather than hard-coded.

use std::process::Command;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{PROJECT_SOURCE, ProjectMode};
use crate::support::{FIXTURE_DYLINT_CONFIG, pinned_toolchain};

/// The exact workspace lockfile versions the generated projects pin.
pub(crate) struct LockedVersions {
    /// The `tracing` version both projects must resolve to.
    pub(crate) tracing: String,
    /// The `log` version the `log` project introduces.
    pub(crate) log: String,
}

/// Read the exact `tracing` and `log` versions from the workspace lockfile.
///
/// # Errors
///
/// Return an error when the workspace lockfile cannot be read or lacks either
/// package.
pub(crate) fn workspace_versions(workspace_root: &Utf8Path) -> anyhow::Result<LockedVersions> {
    Ok(LockedVersions {
        tracing: lockfile_version(workspace_root, "tracing")?,
        log: lockfile_version(workspace_root, "log")?,
    })
}

/// Return `package`'s pinned version from the `Cargo.lock` under `root`.
///
/// # Errors
///
/// Return an error when the lockfile is absent, is not valid TOML, has no
/// `package` array, or does not list `package`.
pub(crate) fn lockfile_version(root: &Utf8Path, package: &str) -> anyhow::Result<String> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("failed to open `{root}` to read its lockfile"))?;
    let text = directory
        .read_to_string("Cargo.lock")
        .with_context(|| format!("failed to read `{root}/Cargo.lock`"))?;
    let document: toml::Value =
        toml::from_str(&text).with_context(|| format!("`{root}/Cargo.lock` is not valid TOML"))?;
    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .with_context(|| format!("`{root}/Cargo.lock` has no `package` array"))?;
    packages
        .iter()
        .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(package))
        .and_then(|entry| entry.get("version").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .with_context(|| format!("`{root}/Cargo.lock` does not list `{package}`"))
}

/// Create and populate one generated project beneath `fixture_root`.
///
/// The project directory is created here so the caller never holds an ambient
/// path that does not exist yet. Returns the generated project's root.
///
/// # Errors
///
/// Return an error when the directories or project files cannot be written.
pub(crate) fn write_project(
    fixture_root: &Utf8Path,
    mode: ProjectMode,
    versions: &LockedVersions,
) -> anyhow::Result<Utf8PathBuf> {
    let fixture = Dir::open_ambient_dir(fixture_root, ambient_authority())
        .with_context(|| format!("failed to open the matrix fixture root `{fixture_root}`"))?;
    fixture
        .create_dir_all(mode.package_name())
        .context("failed to create the generated project directory")?;
    let project_root = fixture_root.join(mode.package_name());
    let directory = Dir::open_ambient_dir(&project_root, ambient_authority())
        .with_context(|| format!("failed to open the generated project `{project_root}`"))?;
    directory
        .create_dir_all("src")
        .context("failed to create the generated project's `src` directory")?;
    directory
        .write("Cargo.toml", project_manifest(mode, versions))
        .context("failed to write the generated project manifest")?;
    directory
        .write("dylint.toml", FIXTURE_DYLINT_CONFIG)
        .context("failed to write the generated project's `dylint.toml`")?;
    directory
        .write("src/lib.rs", PROJECT_SOURCE)
        .context("failed to write the generated project source")?;
    Ok(project_root)
}

/// Render the generated project's manifest with exact dependency pins.
fn project_manifest(mode: ProjectMode, versions: &LockedVersions) -> String {
    // Rendering the optional dependency up front keeps the manifest a single
    // allocation and avoids growing a `String` with `format!`.
    let log_dependency = match mode {
        ProjectMode::WithLog => format!("log = \"={}\"\n", versions.log),
        ProjectMode::WithoutLog => String::new(),
    };
    format!(
        concat!(
            "[package]\n",
            "name = \"{}\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[workspace]\n",
            "\n",
            "[dependencies]\n",
            "tracing = {{ version = \"={}\", default-features = false, features = [{}] }}\n",
            "{}",
        ),
        mode.package_name(),
        versions.tracing,
        mode.tracing_features(),
        log_dependency,
    )
}

/// Resolve and write one generated project's `Cargo.lock`.
///
/// The shared default Cargo cache and compiler wrappers are inherited so
/// admission still applies. `RUSTFLAGS` is removed because an outer gate's
/// warning denial must not reach dependency resolution.
///
/// # Errors
///
/// Return an error when the command cannot run or exits unsuccessfully.
pub(crate) fn generate_lockfile(project_root: &Utf8Path) -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(project_root.as_std_path())
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .env_remove("RUSTFLAGS")
        .output()
        .context("failed to execute `cargo generate-lockfile`")?;
    if !output.status.success() {
        anyhow::bail!(
            "`cargo generate-lockfile` failed in `{project_root}`:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Assert both generated lockfiles pin the workspace's exact `tracing` version.
///
/// # Errors
///
/// Return an error when a generated lockfile cannot be read or disagrees with
/// the workspace lock.
pub(crate) fn assert_versions_agree(
    plain_root: &Utf8Path,
    with_log_root: &Utf8Path,
    versions: &LockedVersions,
) -> anyhow::Result<()> {
    let plain = lockfile_version(plain_root, "tracing")?;
    let with_log = lockfile_version(with_log_root, "tracing")?;
    anyhow::ensure!(
        plain == versions.tracing,
        "the plain project pinned tracing {plain}, but the workspace lock pins {}",
        versions.tracing
    );
    anyhow::ensure!(
        with_log == versions.tracing,
        "the `log` project pinned tracing {with_log}, but the workspace lock pins {}",
        versions.tracing
    );
    Ok(())
}
