//! Tests for workspace detection and checkout orchestration.

use super::*;
use crate::dirs::{MockBaseDirs, SystemBaseDirs};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use std::path::PathBuf;
use tempfile::TempDir;

/// A temporary directory converted to a UTF-8 path for workspace tests.
struct TempWorkspace {
    _temp: TempDir,
    path: Utf8PathBuf,
    dir: Dir,
}

#[fixture]
fn temp_workspace() -> TempWorkspace {
    let temp = TempDir::new().expect("failed to create temp dir");
    let path = Utf8PathBuf::try_from(temp.path().to_owned()).expect("non-UTF8 temp path");
    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .expect("failed to open temporary workspace directory");
    TempWorkspace {
        _temp: temp,
        path,
        dir,
    }
}

fn write_cargo_toml(dir: &Dir, package_name: &str) {
    dir.write(
        "Cargo.toml",
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
        write_cargo_toml(&temp_workspace.dir, name);
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
    write_cargo_toml(&temp_workspace.dir, "whitaker");
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
    temp_workspace
        .dir
        .create_dir("clone_target")
        .expect("failed to create clone dir");

    let action = decide_workspace_action(&temp_workspace.path, &clone_dir, true);

    assert_eq!(action, WorkspaceAction::UpdateAt(clone_dir));
}

#[rstest]
fn decide_workspace_action_uses_existing_when_no_update(temp_workspace: TempWorkspace) {
    let clone_dir = temp_workspace.path.join("clone_target");
    temp_workspace
        .dir
        .create_dir("clone_target")
        .expect("failed to create clone dir");

    let action = decide_workspace_action(&temp_workspace.path, &clone_dir, false);

    assert_eq!(action, WorkspaceAction::UseExisting(clone_dir));
}

#[test]
fn ensure_ref_allowed_refuses_current_dir_workspace() {
    let action = WorkspaceAction::UseCurrentDir(Utf8PathBuf::from("/some/whitaker"));
    let err = ensure_ref_allowed(&action, Some("v0.2.5")).expect_err("expected refusal");
    let InstallerError::RefUnsupported { git_ref } = &err else {
        panic!("expected RefUnsupported, got {err:?}");
    };
    assert_eq!(git_ref, "v0.2.5");
    assert_eq!(
        err.to_string(),
        concat!(
            "cannot pin --ref v0.2.5: the current directory is itself a Whitaker ",
            "workspace; run the installer from outside a checkout to pin the suite"
        )
    );

    let InstallerError::RefUnsupported { git_ref } = err.clone() else {
        panic!("cloned error changed variant");
    };
    assert_eq!(git_ref, "v0.2.5");
}

#[test]
fn ensure_ref_allowed_permits_current_dir_without_ref() {
    let action = WorkspaceAction::UseCurrentDir(Utf8PathBuf::from("/some/whitaker"));
    assert!(ensure_ref_allowed(&action, None).is_ok());
}

#[rstest]
#[case::clone(WorkspaceAction::CloneTo(Utf8PathBuf::from("/clone")))]
#[case::update(WorkspaceAction::UpdateAt(Utf8PathBuf::from("/clone")))]
#[case::existing(WorkspaceAction::UseExisting(Utf8PathBuf::from("/clone")))]
fn ensure_ref_allowed_permits_ref_for_managed_clones(#[case] action: WorkspaceAction) {
    assert!(ensure_ref_allowed(&action, Some("v0.2.5")).is_ok());
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
fn resolve_workspace_path_returns_clone_dir_when_not_in_workspace(temp_workspace: TempWorkspace) {
    // Mock returns a data directory inside temp workspace
    let expected_dir = temp_workspace.path.join("data").join("whitaker");
    let mock = mock_dirs_returning(Some(expected_dir.clone().into_std_path_buf()));

    let result = resolve_workspace_path(&mock);

    assert!(result.is_ok());
    assert_eq!(
        result.expect("workspace path should resolve from the mock data directory"),
        expected_dir
    );
}

#[rstest]
fn resolve_workspace_path_errors_when_data_dir_unavailable(temp_workspace: TempWorkspace) {
    let _ = temp_workspace; // Ensure fixture is used
    let mock = mock_dirs_returning(None);

    let result = resolve_workspace_path(&mock);

    assert!(result.is_err());
    let err = result.expect_err("workspace path should fail without a data directory");
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
fn clone_directory_returns_path_from_mock(temp_workspace: TempWorkspace) {
    let expected = temp_workspace.path.join("data").join("whitaker");
    let mock = mock_dirs_returning(Some(expected.clone().into_std_path_buf()));
    assert_eq!(clone_directory(&mock), Some(expected));
}

// Tests for find_workspace_root

fn write_workspace_cargo_toml(dir: &Dir) {
    dir.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n")
        .expect("failed to write workspace Cargo.toml");
}

#[rstest]
fn find_workspace_root_finds_workspace_in_current_dir(temp_workspace: TempWorkspace) {
    write_workspace_cargo_toml(&temp_workspace.dir);
    assert_eq!(
        find_workspace_root(&temp_workspace.path).expect("workspace root should be found"),
        temp_workspace.path
    );
}

#[rstest]
fn find_workspace_root_finds_workspace_in_parent_dir(temp_workspace: TempWorkspace) {
    write_workspace_cargo_toml(&temp_workspace.dir);
    let subdir = temp_workspace.path.join("crates").join("my_crate");
    temp_workspace
        .dir
        .create_dir_all("crates/my_crate")
        .expect("failed to create workspace subdirectories");
    assert_eq!(
        find_workspace_root(&subdir).expect("workspace root should be found from child crate"),
        temp_workspace.path
    );
}

#[rstest]
fn find_workspace_root_errors_when_no_workspace_found(temp_workspace: TempWorkspace) {
    write_cargo_toml(&temp_workspace.dir, "not_a_workspace");
    let result = find_workspace_root(&temp_workspace.path);
    assert!(matches!(
        result.expect_err("non-workspace package should not have a workspace root"),
        InstallerError::WorkspaceNotFound { .. }
    ));
}
