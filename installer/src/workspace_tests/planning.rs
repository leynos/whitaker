//! Tests for workspace planning decisions and clone-directory resolution.
//!
//! These are pure queries over a directory layout, so they need neither a
//! Git remote nor an update; the orchestration tests stay in the parent
//! module because they drive the same code through mocks.

use super::*;

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
