//! Tests for finding a Cargo workspace root from a directory.

use super::*;

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
