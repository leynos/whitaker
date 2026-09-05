//! Tests for workspace-root discovery.

use super::{InstallerError, TempWorkspace, find_workspace_root, temp_workspace, write_cargo_toml};
use cap_std::fs_utf8::Dir;
use rstest::rstest;

fn write_workspace_cargo_toml(dir: &Dir) {
    dir.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n")
        .expect("failed to write workspace Cargo.toml");
}

#[rstest]
fn finds_workspace_in_current_dir(temp_workspace: TempWorkspace) {
    write_workspace_cargo_toml(&temp_workspace.dir);
    assert_eq!(
        find_workspace_root(&temp_workspace.path).expect("workspace root should be found"),
        temp_workspace.path
    );
}

#[rstest]
fn finds_workspace_in_parent_dir(temp_workspace: TempWorkspace) {
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
fn errors_when_no_workspace_is_found(temp_workspace: TempWorkspace) {
    write_cargo_toml(&temp_workspace.dir, "not_a_workspace");
    let result = find_workspace_root(&temp_workspace.path);
    assert!(matches!(
        result.expect_err("non-workspace package should not have a workspace root"),
        InstallerError::WorkspaceNotFound { .. }
    ));
}
