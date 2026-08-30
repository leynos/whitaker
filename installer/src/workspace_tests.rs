//! Tests for workspace discovery and layout.

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
fn temp_workspace() -> std::io::Result<TempWorkspace> {
    let temp = TempDir::new()?;
    let path = Utf8PathBuf::try_from(temp.path().to_owned())
        .map_err(|_| std::io::Error::other("temporary directory path must be UTF-8"))?;
    Ok(TempWorkspace { _temp: temp, path })
}

fn write_cargo_toml(dir: &Utf8Path, package_name: &str) -> std::io::Result<()> {
    let cargo_toml = dir.join("Cargo.toml");
    fs::write(
        cargo_toml,
        format!("[package]\nname = \"{package_name}\"\nversion = \"0.1.0\"\n"),
    )
}

#[rstest]
#[case::whitaker_project(Some("whitaker"), true)]
#[case::other_project(Some("other-project"), false)]
#[case::empty_dir(None, false)]
fn is_whitaker_workspace_detection(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
    #[case] package_name: Option<&str>,
    #[case] expected: bool,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    if let Some(name) = package_name {
        write_cargo_toml(&temp_workspace.path, name).expect("Cargo.toml should be written");
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
fn decide_workspace_action_uses_cwd_when_whitaker(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    write_cargo_toml(&temp_workspace.path, "whitaker").expect("Cargo.toml should be written");
    let clone_dir = Utf8PathBuf::from("/nonexistent/clone/dir");

    let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

    assert_eq!(action, WorkspaceAction::UseCurrentDir(temp_workspace.path));
}

#[rstest]
fn decide_workspace_action_clones_when_empty(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    // temp_workspace.path is empty (no Cargo.toml), clone_dir doesn't exist
    let clone_dir = temp_workspace.path.join("clone_target");

    let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

    assert_eq!(action, WorkspaceAction::CloneTo(clone_dir));
}

#[rstest]
fn decide_workspace_action_updates_when_clone_exists(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    // Create a clone directory (not a whitaker workspace, just exists)
    let clone_dir = temp_workspace.path.join("clone_target");
    fs::create_dir(&clone_dir).expect("failed to create clone dir");

    let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

    assert_eq!(action, WorkspaceAction::UpdateAt(clone_dir));
}

#[rstest]
fn decide_workspace_action_uses_existing_when_no_update(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

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
    mock.expect_whitaker_data().return_const(data_dir);
    mock
}

#[rstest]
fn resolve_workspace_path_returns_clone_dir_when_not_in_workspace(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    // Mock returns a data directory inside temp workspace
    let expected_dir = temp_workspace.path.join("data").join("whitaker");
    let mock = mock_dirs_returning(Some(expected_dir.clone().into_std_path_buf()));

    let result = resolve_workspace_path(&mock);

    assert_eq!(result.expect("workspace path should resolve"), expected_dir);
}

#[rstest]
fn resolve_workspace_path_errors_when_data_dir_unavailable(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    let _ = temp_workspace; // Ensure fixture is used
    let mock = mock_dirs_returning(None);

    let result = resolve_workspace_path(&mock);

    let err = result.expect_err("resolution should fail without a data directory");
    assert!(
        matches!(err, InstallerError::WorkspaceNotFound { .. }),
        "expected WorkspaceNotFound error, got: {err:?}"
    );
}

#[test]
fn clone_directory_returns_none_when_data_dir_unavailable() {
    let mock = mock_dirs_returning(None);
    assert!(clone_directory(&mock).is_none());
}

#[rstest]
fn clone_directory_returns_path_from_mock(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    let expected = temp_workspace.path.join("data").join("whitaker");
    let mock = mock_dirs_returning(Some(expected.clone().into_std_path_buf()));
    assert_eq!(clone_directory(&mock), Some(expected));
}

// Tests for find_workspace_root

fn write_workspace_cargo_toml(dir: &Utf8Path) -> std::io::Result<()> {
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )
}

#[rstest]
fn find_workspace_root_finds_workspace_in_current_dir(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    write_workspace_cargo_toml(&temp_workspace.path).expect("Cargo.toml should be written");
    assert_eq!(
        find_workspace_root(&temp_workspace.path).expect("workspace root should be found"),
        temp_workspace.path
    );
}

#[rstest]
fn find_workspace_root_finds_workspace_in_parent_dir(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    write_workspace_cargo_toml(&temp_workspace.path).expect("Cargo.toml should be written");
    let subdir = temp_workspace.path.join("crates").join("my_crate");
    fs::create_dir_all(&subdir).expect("failed to create subdirs");
    assert_eq!(
        find_workspace_root(&subdir).expect("workspace root should be found"),
        temp_workspace.path
    );
}

#[rstest]
fn find_workspace_root_errors_when_no_workspace_found(
    #[from(temp_workspace)] temp_workspace_res: std::io::Result<TempWorkspace>,
) {
    let temp_workspace = temp_workspace_res.expect("temporary workspace should be created");

    write_cargo_toml(&temp_workspace.path, "not_a_workspace")
        .expect("Cargo.toml should be written");
    let result = find_workspace_root(&temp_workspace.path);
    assert!(matches!(
        result.expect_err("workspace lookup should fail"),
        InstallerError::WorkspaceNotFound { .. }
    ));
}
