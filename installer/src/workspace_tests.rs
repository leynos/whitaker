//! Tests for workspace detection, planning and clone reuse.
//!
//! Split from `workspace.rs` to keep each file inside the repository's
//! 400-line limit; the module they exercise is unchanged.

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

/// A real clone, so the reuse and update paths can be exercised against git
/// rather than against a description of it.
///
/// The two defects these guard were both invisible to any test that did not
/// run git: a pull against a detached `HEAD` fails, and a branch name resolves
/// to a stale local branch that a fetch never moves.
fn init_clone(root: &Utf8Path) -> Option<(Utf8PathBuf, Utf8PathBuf)> {
    let git = |args: &[&str], cwd: &Utf8Path| -> bool {
        std::process::Command::new("git")
            .args(args)
            .current_dir(cwd.as_std_path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    };
    let origin = root.join("origin");
    let clone = root.join("clone");
    fs::create_dir_all(&origin).ok()?;
    if !git(&["init", "-q", "-b", "main", "."], &origin) {
        return None;
    }
    git(&["config", "user.email", "test@example.invalid"], &origin);
    git(&["config", "user.name", "Test"], &origin);
    fs::write(origin.join("file"), "one").ok()?;
    git(&["add", "-A"], &origin);
    git(&["commit", "-qm", "one"], &origin);
    if !git(&["clone", "-q", origin.as_str(), clone.as_str()], root) {
        return None;
    }
    Some((origin, clone))
}

#[rstest]
fn an_unpinned_update_recovers_a_clone_left_detached(temp_workspace: TempWorkspace) {
    // The defect this guards: one pinned install leaves HEAD detached, and
    // every later update in that clone then runs `git pull` there, where git
    // refuses with "You are not currently on a branch". Without the recovery
    // a single pin would break the cache permanently.
    let Some((_origin, clone)) = init_clone(&temp_workspace.path) else {
        return; // git unavailable; the assertion needs a real repository.
    };
    crate::git::checkout_ref(&clone, &"main".try_into().expect("valid reference"))
        .expect("checkout should succeed");
    assert!(
        crate::git::is_detached_head(&clone).expect("HEAD should be readable"),
        "the pin should have left HEAD detached"
    );

    update_existing_clone(&clone, &WorkspacePlan::updating(true)).expect("update should recover");

    assert!(
        !crate::git::is_detached_head(&clone).expect("HEAD should be readable"),
        "the update should have returned to a branch"
    );
}

#[rstest]
fn a_pinned_update_never_pulls(temp_workspace: TempWorkspace) {
    // A pinned plan must not pull at all, because checkout_ref fetches for
    // itself and a pull would fail on the detached HEAD a previous pin left.
    let Some((_origin, clone)) = init_clone(&temp_workspace.path) else {
        return;
    };
    crate::git::checkout_ref(&clone, &"main".try_into().expect("valid reference"))
        .expect("checkout should succeed");
    let plan = WorkspacePlan {
        should_update: true,
        suite_ref: Some("main".try_into().expect("valid reference")),
    };

    let result = update_existing_clone(&clone, &plan);

    assert!(result.is_ok(), "{result:?}");
    assert!(
        crate::git::is_detached_head(&clone).expect("HEAD should be readable"),
        "a pinned plan should leave the pin in place"
    );
}

#[rstest]
fn an_unpinned_reuse_does_not_inherit_a_previous_pin(temp_workspace: TempWorkspace) {
    // `--no-update` after a pin would otherwise silently mean "somebody
    // else's pin" rather than "the suite as it stands".
    let Some((_origin, clone)) = init_clone(&temp_workspace.path) else {
        return;
    };
    crate::git::checkout_ref(&clone, &"main".try_into().expect("valid reference"))
        .expect("checkout should succeed");

    reuse_existing_clone(&clone, &WorkspacePlan::updating(false)).expect("reuse should recover");

    assert!(
        !crate::git::is_detached_head(&clone).expect("HEAD should be readable"),
        "reuse should have returned to a branch"
    );
}

#[rstest]
fn a_local_pin_does_not_require_origin(temp_workspace: TempWorkspace) {
    // A local pin must remain usable with `--no-update` when the origin is
    // unavailable, rather than turning a cached checkout into a network call.
    let Some((_origin, clone)) = init_clone(&temp_workspace.path) else {
        return;
    };
    let changed_origin = std::process::Command::new("git")
        .args(["remote", "set-url", "origin", "/unavailable-origin"])
        .current_dir(clone.as_std_path())
        .status()
        .expect("set-url should run");
    assert!(changed_origin.success(), "set-url should succeed");

    crate::git::checkout_ref(&clone, &"main".try_into().expect("valid reference"))
        .expect("checkout should succeed");
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
