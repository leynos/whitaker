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
        WorkspaceAction::UseCurrentDir(dir) | WorkspaceAction::UseExisting(dir) => dir,
        WorkspaceAction::CloneTo(dir) => {
            crate::git::clone_repository(&dir)?;
            dir
        }
        WorkspaceAction::UpdateAt(dir) => {
            crate::git::update_repository(&dir)?;
            dir
        }
    };

    if let Some(reference) = plan.suite_ref.as_ref() {
        crate::git::checkout_ref(&dir, reference)?;
    }
    Ok(dir)
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
            reference: reference.as_str().to_owned(),
            path: dir.to_string(),
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
mod tests {
    use super::*;
    use crate::dirs::{MockBaseDirs, SystemBaseDirs};
    use rstest::{fixture, rstest};
    use std::fs;
    use std::path::PathBuf;
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

    // -------------------------------------------------------------------------
    // Behavioural tests for workspace orchestration with mocked dependencies
    // -------------------------------------------------------------------------

    fn mock_dirs_returning(data_dir: Option<PathBuf>) -> MockBaseDirs {
        let mut mock = MockBaseDirs::new();
        mock.expect_whitaker_data_dir().return_const(data_dir);
        mock
    }

    #[rstest]
    fn resolve_workspace_path_returns_clone_dir_when_not_in_workspace(
        temp_workspace: TempWorkspace,
    ) {
        // Mock returns a data directory inside temp workspace
        let expected_dir = temp_workspace.path.join("data").join("whitaker");
        let mock = mock_dirs_returning(Some(expected_dir.clone().into_std_path_buf()));

        let result = resolve_workspace_path(&mock);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_dir);
    }

    #[rstest]
    fn resolve_workspace_path_errors_when_data_dir_unavailable(temp_workspace: TempWorkspace) {
        let _ = temp_workspace; // Ensure fixture is used
        let mock = mock_dirs_returning(None);

        let result = resolve_workspace_path(&mock);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, InstallerError::WorkspaceNotFound { .. }),
            "expected WorkspaceNotFound error, got: {err:?}"
        );
    }

    #[rstest]
    fn plan_refuses_a_pin_inside_a_whitaker_workspace(temp_workspace: TempWorkspace) {
        // Checking out a reference here would move the caller's own working
        // tree, discarding whatever they were doing. Refused rather than
        // done, and the error says where to run it instead.
        write_cargo_toml(&temp_workspace.path, "whitaker");
        let clone_dir = temp_workspace.path.join("clone_target");
        let plan = WorkspacePlan {
            should_update: false,
            suite_ref: Some("v0.2.7".try_into().expect("valid reference")),
        };

        let result = plan_workspace_action(&temp_workspace.path, &clone_dir, &plan);

        let error = result.expect_err("a pin here should be refused");
        assert!(
            matches!(error, InstallerError::SuitePinInWorkspace { .. }),
            "expected SuitePinInWorkspace, got: {error:?}"
        );
        let rendered = error.to_string();
        assert!(rendered.contains("v0.2.7"), "{rendered}");
    }

    #[rstest]
    fn plan_uses_a_whitaker_workspace_when_nothing_is_pinned(temp_workspace: TempWorkspace) {
        // The counterpart: without a pin the current workspace is used, which
        // is what a Whitaker developer running the installer in-tree expects.
        write_cargo_toml(&temp_workspace.path, "whitaker");
        let clone_dir = temp_workspace.path.join("clone_target");

        let action = plan_workspace_action(
            &temp_workspace.path,
            &clone_dir,
            &WorkspacePlan::updating(false),
        )
        .expect("no pin, so no refusal");

        assert_eq!(
            action,
            WorkspaceAction::UseCurrentDir(temp_workspace.path.clone())
        );
    }

    #[rstest]
    fn plan_clones_and_pins_from_outside_a_workspace(temp_workspace: TempWorkspace) {
        // The CI case: not in a Whitaker checkout, so the pin applies to the
        // installer's own clone and nothing of the caller's is touched.
        let clone_dir = temp_workspace.path.join("clone_target");
        let plan = WorkspacePlan {
            should_update: true,
            suite_ref: Some("v0.2.7".try_into().expect("valid reference")),
        };

        let action = plan_workspace_action(&temp_workspace.path, &clone_dir, &plan)
            .expect("a pin outside a workspace is fine");

        assert_eq!(action, WorkspaceAction::CloneTo(clone_dir));
    }

    #[rstest]
    fn the_default_plan_pins_nothing_and_does_not_update() {
        // The default has to stay the branch tip: pinning costs a source
        // build, because prebuilt artefacts exist only for the tip.
        let plan = WorkspacePlan::default();

        assert!(plan.suite_ref.is_none());
        assert!(!plan.should_update);
    }

    #[test]
    fn clone_directory_returns_none_when_data_dir_unavailable() {
        let mock = mock_dirs_returning(None);
        assert!(clone_directory(&mock).is_none());
    }

    #[rstest]
    fn clone_directory_returns_path_from_mock(temp_workspace: TempWorkspace) {
        let expected = temp_workspace.path.join("data").join("whitaker");
        let mock = mock_dirs_returning(Some(expected.clone().into_std_path_buf()));
        assert_eq!(clone_directory(&mock), Some(expected));
    }

    // Tests for find_workspace_root

    fn write_workspace_cargo_toml(dir: &Utf8Path) {
        fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .expect("failed to write workspace Cargo.toml");
    }

    #[rstest]
    fn find_workspace_root_finds_workspace_in_current_dir(temp_workspace: TempWorkspace) {
        write_workspace_cargo_toml(&temp_workspace.path);
        assert_eq!(
            find_workspace_root(&temp_workspace.path).unwrap(),
            temp_workspace.path
        );
    }

    #[rstest]
    fn find_workspace_root_finds_workspace_in_parent_dir(temp_workspace: TempWorkspace) {
        write_workspace_cargo_toml(&temp_workspace.path);
        let subdir = temp_workspace.path.join("crates").join("my_crate");
        fs::create_dir_all(&subdir).expect("failed to create subdirs");
        assert_eq!(find_workspace_root(&subdir).unwrap(), temp_workspace.path);
    }

    #[rstest]
    fn find_workspace_root_errors_when_no_workspace_found(temp_workspace: TempWorkspace) {
        write_cargo_toml(&temp_workspace.path, "not_a_workspace");
        let result = find_workspace_root(&temp_workspace.path);
        assert!(matches!(
            result.unwrap_err(),
            InstallerError::WorkspaceNotFound { .. }
        ));
    }
}
