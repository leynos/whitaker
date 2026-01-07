//! Workspace detection and path resolution.
//!
//! This module provides utilities for detecting whether the current directory
//! is a Whitaker workspace and for resolving platform-specific clone locations.

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// Repository URL for cloning Whitaker.
pub const WHITAKER_REPO_URL: &str = "https://github.com/leynos/whitaker";

/// Expected package name in Cargo.toml to identify a Whitaker workspace.
const WHITAKER_PACKAGE_NAME: &str = "whitaker";

/// Checks whether the given directory contains a Whitaker workspace.
///
/// A Whitaker workspace is identified by a `Cargo.toml` file with
/// `package.name = "whitaker"`.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8Path;
/// use whitaker_installer::workspace::is_whitaker_workspace;
///
/// let dir = Utf8Path::new("/path/to/project");
/// if is_whitaker_workspace(dir) {
///     println!("This is a Whitaker workspace");
/// }
/// ```
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
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::workspace::clone_directory;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialise directories");
/// if let Some(dir) = clone_directory(&dirs) {
///     println!("Whitaker will be cloned to: {dir}");
/// }
/// ```
pub fn clone_directory(dirs: &dyn BaseDirs) -> Option<Utf8PathBuf> {
    dirs.whitaker_data_dir()
        .and_then(|p| Utf8PathBuf::try_from(p).ok())
}

/// Describes the action needed to establish a Whitaker workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceAction {
    /// The current directory is already a Whitaker workspace.
    UseCurrentDir(Utf8PathBuf),
    /// The repository needs to be cloned to the given directory.
    CloneTo(Utf8PathBuf),
    /// The existing repository at the given directory should be updated.
    UpdateAt(Utf8PathBuf),
    /// The repository exists but update was not requested.
    UseExisting(Utf8PathBuf),
}

/// Determines what action is needed to establish a Whitaker workspace.
///
/// This is a pure function that examines the current directory and clone
/// directory state to decide what operation (if any) is needed.
///
/// # Arguments
///
/// * `cwd` - The current working directory.
/// * `clone_dir` - The platform-specific clone directory.
/// * `update` - Whether to update an existing clone.
///
/// # Examples
///
/// ```
/// use camino::Utf8PathBuf;
/// use whitaker_installer::workspace::{decide_workspace_action, WorkspaceAction};
///
/// let cwd = Utf8PathBuf::from("/some/random/dir");
/// let clone_dir = Utf8PathBuf::from("/home/user/.local/share/whitaker");
///
/// match decide_workspace_action(&cwd, &clone_dir, true) {
///     WorkspaceAction::UseCurrentDir(dir) => println!("Using CWD: {dir}"),
///     WorkspaceAction::CloneTo(dir) => println!("Need to clone to: {dir}"),
///     WorkspaceAction::UpdateAt(dir) => println!("Need to update: {dir}"),
///     WorkspaceAction::UseExisting(dir) => println!("Using existing: {dir}"),
/// }
/// ```
pub fn decide_workspace_action(
    cwd: &Utf8Path,
    clone_dir: &Utf8Path,
    update: bool,
) -> WorkspaceAction {
    if is_whitaker_workspace(cwd) {
        WorkspaceAction::UseCurrentDir(cwd.to_owned())
    } else if clone_dir.exists() {
        if update {
            WorkspaceAction::UpdateAt(clone_dir.to_owned())
        } else {
            WorkspaceAction::UseExisting(clone_dir.to_owned())
        }
    } else {
        WorkspaceAction::CloneTo(clone_dir.to_owned())
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
/// * `dirs` - Directory resolver for platform-specific paths.
/// * `update` - If `true` and the repository already exists, runs `git pull`.
///
/// # Errors
///
/// Returns an error if:
/// - The clone directory cannot be determined
/// - Cloning or updating fails
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::workspace::ensure_workspace;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialise directories");
/// // Ensure workspace exists, updating if it already exists
/// let workspace_path = ensure_workspace(&dirs, true)?;
/// println!("Workspace available at: {workspace_path}");
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn ensure_workspace(dirs: &dyn BaseDirs, update: bool) -> Result<Utf8PathBuf> {
    let cwd = current_dir_utf8()?;
    let clone_dir = clone_directory(dirs).ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine data directory for cloning".to_owned(),
    })?;

    match decide_workspace_action(&cwd, &clone_dir, update) {
        WorkspaceAction::UseCurrentDir(dir) | WorkspaceAction::UseExisting(dir) => Ok(dir),
        WorkspaceAction::CloneTo(dir) => {
            crate::git::clone_repository(&dir)?;
            Ok(dir)
        }
        WorkspaceAction::UpdateAt(dir) => {
            crate::git::update_repository(&dir)?;
            Ok(dir)
        }
    }
}

/// Returns the workspace path without performing any side effects.
///
/// If the current directory is a Whitaker workspace, returns it.
/// Otherwise returns the platform-specific clone directory (which may
/// not exist yet).
///
/// This is useful for dry-run mode where we want to show what would happen
/// without actually cloning or updating the repository.
///
/// # Arguments
///
/// * `dirs` - Directory resolver for platform-specific paths.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::workspace::resolve_workspace_path;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialise directories");
/// let workspace_path = resolve_workspace_path(&dirs)?;
/// println!("Would use workspace at: {workspace_path}");
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn resolve_workspace_path(dirs: &dyn BaseDirs) -> Result<Utf8PathBuf> {
    let cwd = current_dir_utf8()?;

    if is_whitaker_workspace(&cwd) {
        return Ok(cwd);
    }

    clone_directory(dirs).ok_or_else(|| InstallerError::WorkspaceNotFound {
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
    use crate::dirs::SystemBaseDirs;
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
    #[case::whitaker_project(Some("whitaker"), true)]
    #[case::other_project(Some("other-project"), false)]
    #[case::empty_dir(None, false)]
    fn is_whitaker_workspace_detection(
        temp_workspace: TempWorkspace,
        #[case] package_name: Option<&str>,
        #[case] expected: bool,
    ) {
        if let Some(name) = package_name {
            write_cargo_toml(&temp_workspace.path, name);
        }
        assert_eq!(is_whitaker_workspace(&temp_workspace.path), expected);
    }

    #[test]
    fn clone_directory_returns_some_on_supported_platforms() {
        // This test may fail on unsupported platforms, but should pass on
        // Linux, macOS, and Windows.
        let dirs = SystemBaseDirs::new().expect("failed to create SystemBaseDirs");
        let dir = clone_directory(&dirs);
        assert!(dir.is_some(), "expected clone_directory to return Some");
        assert!(
            dir.as_ref()
                .is_some_and(|p| p.as_str().contains("whitaker")),
            "expected path to contain 'whitaker'"
        );
    }

    #[rstest]
    fn decide_workspace_action_uses_cwd_when_whitaker(temp_workspace: TempWorkspace) {
        write_cargo_toml(&temp_workspace.path, "whitaker");
        let clone_dir = Utf8PathBuf::from("/nonexistent/clone/dir");

        let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

        assert_eq!(action, WorkspaceAction::UseCurrentDir(temp_workspace.path));
    }

    #[rstest]
    fn decide_workspace_action_clones_when_empty(temp_workspace: TempWorkspace) {
        // temp_workspace.path is empty (no Cargo.toml), clone_dir doesn't exist
        let clone_dir = temp_workspace.path.join("clone_target");

        let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

        assert_eq!(action, WorkspaceAction::CloneTo(clone_dir));
    }

    #[rstest]
    fn decide_workspace_action_updates_when_clone_exists(temp_workspace: TempWorkspace) {
        // Create a clone directory (not a whitaker workspace, just exists)
        let clone_dir = temp_workspace.path.join("clone_target");
        fs::create_dir(&clone_dir).expect("failed to create clone dir");

        let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

        assert_eq!(action, WorkspaceAction::UpdateAt(clone_dir));
    }

    #[rstest]
    fn decide_workspace_action_uses_existing_when_no_update(temp_workspace: TempWorkspace) {
        let clone_dir = temp_workspace.path.join("clone_target");
        fs::create_dir(&clone_dir).expect("failed to create clone dir");

        let action = decide_workspace_action(&temp_workspace.path, &clone_dir, false);

        assert_eq!(action, WorkspaceAction::UseExisting(clone_dir));
    }
}
