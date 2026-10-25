//! Compile-time regression guard for the `dylint-driver`-disabled build.
//!
//! The crate is a Dylint lint library: the lint logic lives behind the
//! optional `dylint-driver` feature, while the default feature set is empty so
//! the crate can be built by tools that only need a plain library. Issue #322
//! removed the private stub that previously existed solely to keep that
//! empty build warning-free. This test re-checks the configuration the stub
//! existed for: `cargo check --no-default-features --lib` must still succeed
//! with warnings denied, now without relying on that stub.
//!
//! The check is deliberately restricted to the library target: the crate's
//! integration test binaries are dylint harnesses that require `cargo-dylint`
//! and `dylint-link`, which must not be assumed here. The nested `cargo`
//! invocation therefore inherits the outer `RUSTFLAGS` (so `-D warnings` from
//! the Makefile gate applies) but uses an isolated target directory so it never
//! contends with the outer build.

use std::process::Command;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde_json::Value;
use tempfile::TempDir;

/// Top-level name of the package under test.
const CRATE: &str = "no_std_fs_operations";

/// Run `cargo check --no-default-features --lib` for this crate and assert
/// that the build succeeds with warnings denied (via inherited `RUSTFLAGS`).
///
/// The workspace manifest is located by walking up from this test crate's
/// manifest directory, mirroring `integration_exclusion.rs`, so the nested
/// invocation resolves the same workspace this test builds under.
///
/// # Errors
///
/// Return an error if the workspace cannot be located, the nested Cargo command
/// fails, or it emits compiler diagnostics.
#[test]
fn crate_builds_without_dylint_driver_feature() -> anyhow::Result<()> {
    let workspace_root = workspace_root()?;
    let target_dir = TempDir::new().context("failed to create isolated target directory")?;

    let output = Command::new("cargo")
        .arg("check")
        .arg("--frozen")
        .arg("--package")
        .arg(CRATE)
        .arg("--no-default-features")
        .arg("--lib")
        .arg("--message-format=json")
        .current_dir(workspace_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir.path())
        .output()
        .context("failed to execute nested cargo check")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "`cargo check --no-default-features --lib` for `{CRATE}` failed (it must compile \
             without the `dylint-driver` feature now that the no-driver stub is gone):\n{stderr}"
        );
    }

    let diagnostics = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-message")
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        anyhow::bail!(
            "`cargo check --no-default-features --lib` for `{CRATE}` emitted compiler diagnostics \
             under `-D warnings`: {diagnostics:#?}"
        );
    }

    Ok(())
}

/// Find the workspace root containing this crate's manifest.
///
/// # Errors
///
/// Return an error when no workspace manifest exists above `CARGO_MANIFEST_DIR`
/// or a candidate directory or manifest cannot be read.
fn workspace_root() -> anyhow::Result<Utf8PathBuf> {
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
/// # Errors
///
/// Return an error when the directory cannot be opened or its manifest exists
/// but cannot be read. Return `false` only when `Cargo.toml` is absent or does
/// not declare a workspace.
fn is_workspace_root(candidate: &Utf8Path) -> anyhow::Result<bool> {
    let directory = Dir::open_ambient_dir(candidate, ambient_authority())
        .with_context(|| format!("failed to open candidate directory `{candidate}`"))?;
    let manifest = match directory.read_to_string("Cargo.toml") {
        Ok(manifest) => manifest,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read workspace manifest in `{candidate}`"));
        }
    };
    Ok(manifest.contains("[workspace]"))
}

#[cfg(test)]
mod tests {
    //! Check workspace discovery's distinction between absence and I/O failure.

    use cap_std::{ambient_authority, fs_utf8::Dir};
    use rstest::rstest;
    use tempfile::TempDir;

    use super::is_workspace_root;

    /// Return a UTF-8 path for a temporary directory.
    ///
    /// # Errors
    ///
    /// Return an error if the temporary directory cannot be created or its path
    /// cannot be represented as UTF-8.
    fn temporary_directory() -> anyhow::Result<(TempDir, camino::Utf8PathBuf)> {
        let temporary_directory = TempDir::new()?;
        let path = camino::Utf8PathBuf::from_path_buf(temporary_directory.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))?;
        Ok((temporary_directory, path))
    }

    /// Recognize a readable manifest that declares a workspace.
    ///
    /// # Errors
    ///
    /// Return an error if fixture setup fails or workspace discovery reports an
    /// I/O failure.
    #[rstest]
    fn readable_workspace_manifest_is_recognized() -> anyhow::Result<()> {
        let (_temporary_directory, path) = temporary_directory()?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;
        directory.write("Cargo.toml", "[workspace]\nmembers = []\n")?;

        assert!(is_workspace_root(&path)?);
        Ok(())
    }

    /// Treat a missing or non-workspace manifest as an ordinary non-match.
    ///
    /// # Errors
    ///
    /// Return an error if fixture setup fails or workspace discovery reports an
    /// I/O failure.
    #[rstest]
    fn absent_or_non_workspace_manifest_is_not_a_root() -> anyhow::Result<()> {
        let (_temporary_directory, path) = temporary_directory()?;
        assert!(!is_workspace_root(&path)?);

        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;
        directory.write("Cargo.toml", "[package]\nname = \"fixture\"\n")?;
        assert!(!is_workspace_root(&path)?);
        Ok(())
    }

    /// Propagate failure to open a candidate path as a directory.
    ///
    /// # Errors
    ///
    /// Return an error if fixture setup fails or the expected I/O failure is not
    /// reported.
    #[rstest]
    fn directory_open_failure_is_reported() -> anyhow::Result<()> {
        let (_temporary_directory, path) = temporary_directory()?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;
        directory.write("not-a-directory", "fixture")?;
        let candidate = path.join("not-a-directory");

        let error = is_workspace_root(&candidate)
            .expect_err("opening a regular file as a directory should fail");
        assert!(
            error
                .to_string()
                .contains("failed to open candidate directory")
        );
        Ok(())
    }

    /// Propagate failure to read a manifest path that is a directory.
    ///
    /// # Errors
    ///
    /// Return an error if fixture setup fails or the expected I/O failure is not
    /// reported.
    #[rstest]
    fn unreadable_manifest_is_reported() -> anyhow::Result<()> {
        let (_temporary_directory, path) = temporary_directory()?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;
        directory.create_dir("Cargo.toml")?;

        let error =
            is_workspace_root(&path).expect_err("reading a directory as a manifest should fail");
        assert!(
            error
                .to_string()
                .contains("failed to read workspace manifest")
        );
        Ok(())
    }
}
