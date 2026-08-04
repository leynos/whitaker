//! Workspace detection and path resolution.
//!
//! This module provides utilities for detecting whether the current directory
//! is a Whitaker workspace and for resolving platform-specific clone locations.

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// Repository URL for cloning Whitaker.
///
/// Re-exported from [`crate::git`] to preserve the existing public path.
pub use crate::git::WHITAKER_REPO_URL;

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

/// Selects the action needed to establish the Whitaker workspace.
///
/// This is the shared, side-effect-free decision boundary used by real and
/// dry-run installation paths.
///
/// # Errors
///
/// Returns an error when the current directory or managed clone directory
/// cannot be determined.
pub fn resolve_workspace_action(dirs: &dyn BaseDirs, update: bool) -> Result<WorkspaceAction> {
    let cwd = current_dir_utf8()?;
    let clone_dir = clone_directory(dirs).ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine data directory for cloning".to_owned(),
    })?;
    Ok(decide_workspace_action(&cwd, &clone_dir, update))
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

/// The prepared workspace and, when a ref was pinned, its resolved commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCheckout {
    /// Path to the workspace root the install should build from.
    pub root: Utf8PathBuf,
    /// The full commit SHA a `--ref` pin resolved to, if any.
    pub pinned_commit: Option<String>,
    /// The existing detached HEAD reused by an unpinned `--no-update` install.
    pub detached_commit: Option<String>,
    /// The action selected to prepare this workspace.
    pub action: WorkspaceAction,
}

/// Ensures a Whitaker workspace is available, cloning if necessary.
///
/// If the current directory is already a Whitaker workspace, returns its path.
/// Otherwise, clones or updates the repository in the platform-specific data
/// directory. Set `update` to `true` to run `git pull` on existing clones.
///
/// When `git_ref` is `Some`, the managed clone is pinned to that commit-ish
/// (SHA, tag, or branch): the ref is fetched first and checked out as a
/// detached HEAD, falling back to a locally resolvable ref or SHA if fetching
/// fails. Pinning is refused when the current directory is itself a Whitaker
/// workspace, since that would mutate the user's own working tree. The update
/// path first reattaches a detached clone to its default branch, so a previous
/// pin never breaks a later un-pinned install.
///
/// # Errors
///
/// Returns an error if the clone directory cannot be determined, if
/// cloning/updating fails, or if `git_ref` is requested for the current
/// directory workspace.
pub fn ensure_workspace(
    dirs: &dyn BaseDirs,
    update: bool,
    git_ref: Option<&str>,
) -> Result<WorkspaceCheckout> {
    let action = resolve_workspace_action(dirs, update)?;
    ensure_ref_allowed(&action, git_ref)?;

    let root = match &action {
        WorkspaceAction::UseCurrentDir(dir) | WorkspaceAction::UseExisting(dir) => {
            // UseCurrentDir is guaranteed refless by `ensure_ref_allowed`;
            // UseExisting pins without pulling, per the `--no-update` contract.
            dir.clone()
        }
        WorkspaceAction::CloneTo(dir) => {
            crate::git::clone_repository(dir)?;
            dir.clone()
        }
        WorkspaceAction::UpdateAt(dir) => {
            // Reattach before pulling so a prior detached pin cannot break the
            // update, even when no new ref is requested.
            crate::git::ensure_default_branch(dir)?;
            crate::git::update_repository(dir)?;
            dir.clone()
        }
    };
    finalize_workspace_checkout(root, git_ref, action)
}

/// Applies an optional pin and constructs the resulting workspace checkout.
///
/// This helper only owns the common tail after workspace setup. Cloning,
/// updating, and reattachment remain responsibilities of [`ensure_workspace`].
pub(super) fn finalize_workspace_checkout(
    root: Utf8PathBuf,
    git_ref: Option<&str>,
    action: WorkspaceAction,
) -> Result<WorkspaceCheckout> {
    let pinned_commit = pin_if_requested(&root, git_ref)?;
    let detached_commit = inherited_detached_commit(&root, git_ref, &action)?;
    Ok(WorkspaceCheckout {
        root,
        pinned_commit,
        detached_commit,
        action,
    })
}

/// Detects a detached commit inherited by an unpinned no-update checkout.
fn inherited_detached_commit(
    root: &Utf8Path,
    git_ref: Option<&str>,
    action: &WorkspaceAction,
) -> Result<Option<String>> {
    if git_ref.is_none() && matches!(action, WorkspaceAction::UseExisting(_)) {
        return crate::git::detached_head_commit(root);
    }
    Ok(None)
}

impl WorkspaceCheckout {
    /// Returns the commit that a downloaded prebuilt artefact must match.
    #[must_use]
    pub fn expected_git_sha(&self) -> Option<&str> {
        self.pinned_commit
            .as_deref()
            .or(self.detached_commit.as_deref())
    }
}

/// Refuses `--ref` when the current directory is itself a Whitaker workspace.
///
/// Pinning checks out a commit; doing so in the user's own working tree could
/// destroy uncommitted work, so it is rejected rather than attempted.
///
/// # Errors
///
/// Returns [`InstallerError::RefUnsupported`] when `action` uses the current
/// workspace and `git_ref` requests a pin.
pub fn ensure_ref_allowed(action: &WorkspaceAction, git_ref: Option<&str>) -> Result<()> {
    if let (WorkspaceAction::UseCurrentDir(_), Some(git_ref)) = (action, git_ref) {
        return Err(InstallerError::RefUnsupported {
            git_ref: git_ref.to_owned(),
        });
    }
    Ok(())
}

/// Pins the managed clone to `git_ref` when one is requested.
///
/// Returns the resolved commit SHA, or `None` when no ref was requested.
fn pin_if_requested(repo: &Utf8Path, git_ref: Option<&str>) -> Result<Option<String>> {
    match git_ref {
        Some(git_ref) => Ok(Some(pin_to_ref(repo, git_ref)?)),
        None => Ok(None),
    }
}

/// Fetches and checks out `git_ref`, falling back to local resolution offline.
pub(super) fn pin_to_ref(repo: &Utf8Path, git_ref: &str) -> Result<String> {
    let commit = match crate::git::fetch_ref(repo, git_ref) {
        Ok(commit) => commit,
        Err(fetch_error) => crate::git::resolve_commit(repo, git_ref).map_err(|_| fetch_error)?,
    };
    crate::git::checkout_detached(repo, &commit)?;
    Ok(commit)
}

/// Returns the workspace path without performing any side effects.
///
/// If the current directory is a Whitaker workspace, returns it. Otherwise
/// returns the platform-specific clone directory (which may not exist yet).
/// Useful for dry-run mode to show what would happen without cloning.
pub fn resolve_workspace_path(dirs: &dyn BaseDirs) -> Result<Utf8PathBuf> {
    let action = resolve_workspace_action(dirs, false)?;
    Ok(match action {
        WorkspaceAction::UseCurrentDir(dir)
        | WorkspaceAction::CloneTo(dir)
        | WorkspaceAction::UpdateAt(dir)
        | WorkspaceAction::UseExisting(dir) => dir,
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
