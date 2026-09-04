//! Workspace detection and path resolution.
//!
//! This module provides utilities for detecting whether the current directory
//! is a Whitaker workspace and for resolving platform-specific clone locations.

use crate::artefact::suite_ref::SuiteRef;
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

/// How the workspace should be established for one install.
///
/// A pair rather than two arguments because the second only means anything
/// alongside the first: a pin makes the update question moot, since the
/// reference is fetched and checked out either way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlan {
    /// Whether an existing clone should be updated to the branch tip.
    pub should_update: bool,
    /// The reference to build the lint suite from, if the caller pinned one.
    pub suite_ref: Option<SuiteRef>,
}

impl WorkspacePlan {
    /// A plan that updates an existing clone and pins nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::workspace::WorkspacePlan;
    ///
    /// let plan = WorkspacePlan::updating(true);
    /// assert!(plan.should_update);
    /// assert!(plan.suite_ref.is_none());
    /// ```
    #[must_use]
    pub fn updating(should_update: bool) -> Self {
        Self {
            should_update,
            suite_ref: None,
        }
    }
}

/// Ensures a Whitaker workspace is available, cloning if necessary.
///
/// If the current directory is already a Whitaker workspace, returns its path.
/// Otherwise, clones or updates the repository in the platform-specific data
/// directory. Set `should_update` to `true` to run `git pull` on existing
/// clones.
///
/// When the plan carries a suite reference, the clone is fetched and that
/// reference checked out, so the suite is built from it rather than from the
/// branch tip. The fetch happens whether or not `should_update` is set,
/// because a pin is an explicit request for a particular reference and that
/// reference may have been published after the clone was made.
///
/// # Errors
///
/// Returns an error if the clone directory cannot be determined, if
/// cloning, updating or checking out fails, or if a pin was requested while
/// the current directory is itself a Whitaker workspace, since checking out
/// there would move the caller's own working tree.
pub fn ensure_workspace(dirs: &dyn BaseDirs, plan: &WorkspacePlan) -> Result<Utf8PathBuf> {
    let cwd = current_dir_utf8()?;
    let clone_dir = clone_directory(dirs).ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine data directory for cloning".to_owned(),
    })?;

    let action = plan_workspace_action(&cwd, &clone_dir, plan)?;
    let dir = match action {
        WorkspaceAction::UseCurrentDir(dir) => dir,
        WorkspaceAction::UseExisting(dir) => {
            reuse_existing_clone(&dir, plan)?;
            dir
        }
        WorkspaceAction::CloneTo(dir) => {
            crate::git::clone_repository(&dir)?;
            dir
        }
        WorkspaceAction::UpdateAt(dir) => {
            update_existing_clone(&dir, plan)?;
            dir
        }
    };

    if let Some(reference) = plan.suite_ref.as_ref() {
        crate::git::checkout_ref(&dir, reference)?;
    }
    Ok(dir)
}

/// Prepares an existing clone that is about to be updated.
///
/// A pinned plan does not pull at all: `checkout_ref` fetches for itself, and
/// a pull here would run against the detached `HEAD` a previous pinned install
/// left behind, where git refuses with "You are not currently on a branch".
///
/// An unpinned plan restores the default branch first for the same reason,
/// since without it one pinned install would break every later update in that
/// clone.
fn update_existing_clone(dir: &Utf8Path, plan: &WorkspacePlan) -> Result<()> {
    if plan.suite_ref.is_some() {
        return Ok(());
    }
    if crate::git::is_detached_head(dir)? {
        crate::git::restore_default_branch(dir)?;
    }
    crate::git::update_repository(dir)
}

/// Prepares an existing clone that is being reused without updating.
///
/// An unpinned reuse must not silently inherit the commit a previous pinned
/// install left checked out, which is what `--no-update` would otherwise mean
/// after any pin: the caller asked for the suite as it stands in the clone,
/// not for somebody else's pin.
fn reuse_existing_clone(dir: &Utf8Path, plan: &WorkspacePlan) -> Result<()> {
    if plan.suite_ref.is_some() || !crate::git::is_detached_head(dir)? {
        return Ok(());
    }
    crate::git::restore_default_branch(dir)
}

/// Decides the action for a plan, refusing a pin that would move the caller.
///
/// Separate from [`ensure_workspace`] and pure, so the refusal can be tested
/// without a clone, a network, or a change to the process-wide current
/// directory that would race every other test.
///
/// # Errors
///
/// Returns `InstallerError::SuitePinInWorkspace` when a reference is pinned
/// and the current directory is itself a Whitaker workspace, because checking
/// out there would move the caller's own working tree.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_installer::workspace::{plan_workspace_action, WorkspacePlan};
///
/// let plan = WorkspacePlan::updating(false);
/// let action = plan_workspace_action(
///     Utf8Path::new("/somewhere/else"),
///     Utf8Path::new("/data/whitaker"),
///     &plan,
/// );
/// assert!(action.is_ok());
/// ```
pub fn plan_workspace_action(
    cwd: &Utf8Path,
    clone_dir: &Utf8Path,
    plan: &WorkspacePlan,
) -> Result<WorkspaceAction> {
    let action = decide_workspace_action(cwd, clone_dir, plan.should_update);
    if let (WorkspaceAction::UseCurrentDir(dir), Some(reference)) =
        (&action, plan.suite_ref.as_ref())
    {
        return Err(InstallerError::SuitePinInWorkspace {
            reference: reference.clone(),
            path: dir.clone(),
        });
    }
    Ok(action)
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
