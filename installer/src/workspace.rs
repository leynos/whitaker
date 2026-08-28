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
/// Platform paths: Linux `~/.local/share/whitaker`, macOS `~/Library/Application
/// Support/whitaker`, Windows `%LOCALAPPDATA%\whitaker`.
///
/// Returns `None` if the platform's data directory cannot be determined.
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
/// Examines the current directory and clone directory state to decide what
/// operation (if any) is needed. Returns `UseCurrentDir` if `cwd` is a
/// Whitaker workspace, `CloneTo` if `clone_dir` doesn't exist, `UpdateAt`
/// if `update` is true and the clone exists, or `UseExisting` otherwise.
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
/// directory. Set `update` to `true` to run `git pull` on existing clones.
///
/// # Errors
///
/// Returns an error if the clone directory cannot be determined or if
/// cloning/updating fails.
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
/// If the current directory is a Whitaker workspace, returns it. Otherwise
/// returns the platform-specific clone directory (which may not exist yet).
/// Useful for dry-run mode to show what would happen without cloning.
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

/// Find the workspace root by looking for `Cargo.toml` with `[workspace]`.
///
/// # Errors
///
/// Returns an error if the workspace root cannot be determined, or if a
/// `Cargo.toml` file cannot be read or parsed.
pub fn find_workspace_root(start: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut current = start.to_owned();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() && is_cargo_workspace_root(&cargo_toml)? {
            return Ok(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_owned(),
            None => break,
        }
    }

    Err(InstallerError::WorkspaceNotFound {
        reason: "could not find Cargo.toml with [workspace] section".to_owned(),
    })
}

/// Check if a `Cargo.toml` file contains a `[workspace]` section.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
fn is_cargo_workspace_root(cargo_toml: &Utf8Path) -> Result<bool> {
    let contents = std::fs::read_to_string(cargo_toml)?;
    let table = contents
        .parse::<toml::Table>()
        .map_err(|e| InstallerError::InvalidCargoToml {
            path: cargo_toml.to_owned(),
            reason: e.to_string(),
        })?;
    Ok(table.contains_key("workspace"))
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
