//! Tests for workspace detection and checkout orchestration.

use super::*;
use crate::dirs::{MockBaseDirs, SystemBaseDirs};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use std::path::PathBuf;
use std::sync::{
    Arc, Barrier, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;
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

#[test]
fn resolve_workspace_path_errors_when_data_dir_unavailable() {
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

#[derive(Clone)]
struct FixedBaseDirs {
    data_dir: PathBuf,
}

impl BaseDirs for FixedBaseDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        None
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        Some(self.data_dir.clone())
    }
}

struct ConcurrentWorkspaceRepository {
    clone_started: mpsc::Sender<()>,
    release_clone: Mutex<mpsc::Receiver<()>>,
    clone_calls: AtomicUsize,
    update_calls: AtomicUsize,
}

impl WorkspaceRepository for ConcurrentWorkspaceRepository {
    fn clone(&self, target: &Utf8Path) -> Result<()> {
        self.clone_calls.fetch_add(1, Ordering::SeqCst);
        self.clone_started
            .send(())
            .expect("report mock clone start");
        self.release_clone
            .lock()
            .expect("lock mock clone release receiver")
            .recv()
            .expect("release mock clone");
        let parent = target
            .parent()
            .ok_or_else(|| InstallerError::WorkspaceNotFound {
                reason: format!("mock clone target has no parent: {target}"),
            })?;
        let name = target
            .file_name()
            .ok_or_else(|| InstallerError::WorkspaceNotFound {
                reason: format!("mock clone target has no file name: {target}"),
            })?;
        let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
        directory.create_dir(name)?;
        Ok(())
    }

    fn update(&self, _repo: &Utf8Path) -> Result<()> {
        self.update_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn ensure_default_branch(&self, _repo: &Utf8Path) -> Result<()> {
        Ok(())
    }
}

#[rstest]
fn concurrent_workspace_preparation_waits_and_rechecks_action(temp_workspace: TempWorkspace) {
    temp_workspace
        .dir
        .create_dir_all("caller/data")
        .expect("create caller and data directories");
    let cwd = temp_workspace.path.join("caller");
    let clone_dir = temp_workspace.path.join("caller/data/whitaker");
    let dirs = FixedBaseDirs {
        data_dir: clone_dir.clone().into_std_path_buf(),
    };
    let (clone_started_sender, clone_started_receiver) = mpsc::channel();
    let (release_clone_sender, release_clone_receiver) = mpsc::channel();
    let repository = Arc::new(ConcurrentWorkspaceRepository {
        clone_started: clone_started_sender,
        release_clone: Mutex::new(release_clone_receiver),
        clone_calls: AtomicUsize::new(0),
        update_calls: AtomicUsize::new(0),
    });

    let first = {
        let cwd = cwd.clone();
        let dirs = dirs.clone();
        let repository = Arc::clone(&repository);
        thread::spawn(move || {
            let preparation = WorkspacePreparation {
                dirs: &dirs,
                update: true,
                git_ref: None,
            };
            ensure_workspace_from(&cwd, &preparation, &*repository)
        })
    };
    clone_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("first preparation begins cloning while holding the lock");

    let start_barrier = Arc::new(Barrier::new(2));
    let (second_ready_sender, second_ready_receiver) = mpsc::channel();
    let second = {
        let cwd = cwd.clone();
        let dirs = dirs.clone();
        let repository = Arc::clone(&repository);
        let start_barrier = Arc::clone(&start_barrier);
        thread::spawn(move || {
            second_ready_sender
                .send(())
                .expect("report second preparation ready");
            start_barrier.wait();
            let preparation = WorkspacePreparation {
                dirs: &dirs,
                update: true,
                git_ref: None,
            };
            ensure_workspace_from(&cwd, &preparation, &*repository)
        })
    };
    second_ready_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second preparation is ready to contend");
    start_barrier.wait();
    assert!(
        clone_started_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "second preparation must wait for the managed-clone lock"
    );

    release_clone_sender
        .send(())
        .expect("release first preparation clone");
    let first = first
        .join()
        .expect("first preparation thread should not panic")
        .expect("first preparation should succeed");
    let second = second
        .join()
        .expect("second preparation thread should not panic")
        .expect("second preparation should succeed");

    assert_eq!(first.action, WorkspaceAction::CloneTo(clone_dir.clone()));
    assert_eq!(second.action, WorkspaceAction::UpdateAt(clone_dir));
    assert_eq!(repository.clone_calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.update_calls.load(Ordering::SeqCst), 1);
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

#[path = "workspace_lock_workflow_tests.rs"]
mod workspace_lock_workflow_tests;
