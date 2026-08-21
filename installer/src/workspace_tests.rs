//! Tests for workspace detection, planning and clone reuse.
//!
//! Split from `workspace.rs` to keep each file inside the repository's
//! 400-line limit; the module they exercise is unchanged.
use cap_std::{ambient_authority, fs_utf8::Dir};
use crate::dirs::{MockBaseDirs, SystemBaseDirs};
use rstest::{fixture, rstest};
use std::path::PathBuf;
use super::*;
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
fn a_branch_pin_follows_the_remote_rather_than_a_stale_local_branch(temp_workspace: TempWorkspace) {
    // `git checkout main` resolves the *local* branch, which a fetch never
    // fast-forwards, so without preferring `origin/main` a branch pin builds
    // whatever that branch pointed at when the clone was made.
    let Some((origin, clone)) = init_clone(&temp_workspace.path) else {
        return;
    };
    let git = |args: &[&str], cwd: &Utf8Path| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(cwd.as_std_path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    };
    fs::write(origin.join("file"), "two").expect("write should succeed");
    let _ = git(&["commit", "-qam", "two"], &origin);
    let expected = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(origin.as_std_path())
        .output()
        .expect("rev-parse should run");
    let expected = String::from_utf8_lossy(&expected.stdout).trim().to_owned();

    crate::git::checkout_ref(&clone, &"main".try_into().expect("valid reference"))
        .expect("checkout should succeed");

    let actual = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(clone.as_std_path())
        .output()
        .expect("rev-parse should run");
    let actual = String::from_utf8_lossy(&actual.stdout).trim().to_owned();
    assert_eq!(actual, expected, "the pin should follow the remote branch");
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
