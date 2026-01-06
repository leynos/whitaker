//! Workspace detection and path resolution.
//!
//! This module provides utilities for detecting whether the current directory
//! is a Whitaker workspace and for resolving platform-specific clone locations.

use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::path::PathBuf;

/// Repository URL for cloning Whitaker.
pub const WHITAKER_REPO_URL: &str = "https://github.com/leynos/whitaker";

/// Expected package name in Cargo.toml to identify a Whitaker workspace.
const WHITAKER_PACKAGE_NAME: &str = "whitaker";

/// Checks whether the given directory contains a Whitaker workspace.
///
/// A Whitaker workspace is identified by a `Cargo.toml` file with
/// `package.name = "whitaker"`.
pub fn is_whitaker_workspace(dir: &Utf8Path) -> bool {
    let cargo_toml = dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return false;
    }

    let Ok(content) = std::fs::read_to_string(&cargo_toml) else {
        return false;
    };

    let Ok(manifest) = content.parse::<toml::Table>() else {
        return false;
    };

    manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .is_some_and(|name| name == WHITAKER_PACKAGE_NAME)
}

/// Returns the platform-specific directory for cloning Whitaker.
///
/// - Linux: `~/.local/share/whitaker`
/// - macOS: `~/Library/Application Support/whitaker`
/// - Windows: `%LOCALAPPDATA%\whitaker`
///
/// Returns `None` if the platform's data directory cannot be determined.
pub fn clone_directory() -> Option<Utf8PathBuf> {
    dirs::data_dir()
        .and_then(|p| Utf8PathBuf::try_from(p).ok())
        .map(|p| p.join("whitaker"))
}

/// Returns the platform-specific bin directory for wrapper scripts.
///
/// - Unix: `~/.local/bin`
/// - Windows: `%LOCALAPPDATA%\whitaker\bin`
///
/// Returns `None` if the directory cannot be determined.
pub fn wrapper_bin_directory() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        dirs::home_dir().map(|h| h.join(".local").join("bin"))
    }
    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|d| d.join("whitaker").join("bin"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Ensures a Whitaker workspace is available, cloning if necessary.
///
/// If the current directory is already a Whitaker workspace, returns its path.
/// Otherwise, clones or updates the repository in the platform-specific data
/// directory.
///
/// # Arguments
///
/// * `update` - If `true` and the repository already exists, runs `git pull`.
/// * `clone_fn` - Function to clone the repository.
/// * `update_fn` - Function to update an existing repository.
///
/// # Errors
///
/// Returns an error if:
/// - The clone directory cannot be determined
/// - Cloning or updating fails
pub fn ensure_workspace<F, G>(update: bool, clone_fn: F, update_fn: G) -> Result<Utf8PathBuf>
where
    F: FnOnce(&Utf8Path) -> Result<()>,
    G: FnOnce(&Utf8Path) -> Result<()>,
{
    let cwd = current_dir_utf8()?;

    if is_whitaker_workspace(&cwd) {
        return Ok(cwd);
    }

    let clone_dir = clone_directory().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine data directory for cloning".to_owned(),
    })?;

    if clone_dir.exists() {
        if update {
            update_fn(&clone_dir)?;
        }
    } else {
        clone_fn(&clone_dir)?;
    }

    Ok(clone_dir)
}

/// Returns the workspace path without performing any side effects.
///
/// If the current directory is a Whitaker workspace, returns it.
/// Otherwise returns the platform-specific clone directory (which may
/// not exist yet).
///
/// This is useful for dry-run mode where we want to show what would happen
/// without actually cloning or updating the repository.
pub fn resolve_workspace_path() -> Result<Utf8PathBuf> {
    let cwd = current_dir_utf8()?;

    if is_whitaker_workspace(&cwd) {
        return Ok(cwd);
    }

    clone_directory().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine data directory for cloning".to_owned(),
    })
}

/// Gets the current directory as a UTF-8 path.
fn current_dir_utf8() -> Result<Utf8PathBuf> {
    let cwd = std::env::current_dir()?;
    Utf8PathBuf::try_from(cwd).map_err(|e| InstallerError::WorkspaceNotFound {
        reason: format!("current directory is not valid UTF-8: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::fs;
    use tempfile::TempDir;

    /// A temporary directory converted to a UTF-8 path for workspace tests.
    struct TempWorkspace {
        _temp: TempDir,
        path: Utf8PathBuf,
    }

    #[fixture]
    fn temp_workspace() -> TempWorkspace {
        let temp = TempDir::new().expect("failed to create temp dir");
        let path = Utf8PathBuf::try_from(temp.path().to_owned()).expect("non-UTF8 temp path");
        TempWorkspace { _temp: temp, path }
    }

    fn write_cargo_toml(dir: &Utf8Path, package_name: &str) {
        let cargo_toml = dir.join("Cargo.toml");
        fs::write(
            cargo_toml,
            format!("[package]\nname = \"{package_name}\"\nversion = \"0.1.0\"\n"),
        )
        .expect("failed to write Cargo.toml");
    }

    #[rstest]
    fn is_whitaker_workspace_returns_true_for_whitaker(temp_workspace: TempWorkspace) {
        write_cargo_toml(&temp_workspace.path, "whitaker");
        assert!(is_whitaker_workspace(&temp_workspace.path));
    }

    #[rstest]
    fn is_whitaker_workspace_returns_false_for_other_project(temp_workspace: TempWorkspace) {
        write_cargo_toml(&temp_workspace.path, "other-project");
        assert!(!is_whitaker_workspace(&temp_workspace.path));
    }

    #[rstest]
    fn is_whitaker_workspace_returns_false_for_empty_dir(temp_workspace: TempWorkspace) {
        assert!(!is_whitaker_workspace(&temp_workspace.path));
    }

    #[test]
    fn clone_directory_returns_some_on_supported_platforms() {
        // This test may fail on unsupported platforms, but should pass on
        // Linux, macOS, and Windows.
        let dir = clone_directory();
        assert!(dir.is_some(), "expected clone_directory to return Some");
        assert!(
            dir.as_ref().unwrap().as_str().contains("whitaker"),
            "expected path to contain 'whitaker'"
        );
    }

    #[rstest]
    fn ensure_workspace_uses_cwd_when_already_whitaker(temp_workspace: TempWorkspace) {
        write_cargo_toml(&temp_workspace.path, "whitaker");

        // Change to temp directory for this test
        let original_cwd = std::env::current_dir().expect("failed to get cwd");
        std::env::set_current_dir(&*temp_workspace.path).expect("failed to change cwd");

        let result = ensure_workspace(
            false,
            |_| panic!("clone_fn should not be called"),
            |_| panic!("update_fn should not be called"),
        );

        std::env::set_current_dir(original_cwd).expect("failed to restore cwd");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), temp_workspace.path);
    }
}
