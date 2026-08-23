//! Real-Git workflow coverage for managed-clone lock serialization.

use super::super::{
    WorkspaceAction, WorkspacePreparation, WorkspaceRepository, ensure_workspace_from,
};
use crate::dirs::BaseDirs;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[derive(Clone)]
struct ManagedCloneDirs {
    clone_dir: PathBuf,
}

impl BaseDirs for ManagedCloneDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        None
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        Some(self.clone_dir.clone())
    }
}

fn git(dir: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir.as_std_path())
        .output()
        .expect("spawn fixture Git command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn commit_file(dir: &Utf8Path, contents: &str, message: &str) -> String {
    let directory =
        Dir::open_ambient_dir(dir, ambient_authority()).expect("open source fixture capability");
    directory
        .write("fixture.txt", contents)
        .expect("write source fixture file");
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            message,
        ],
    );
    git(dir, &["rev-parse", "HEAD"])
}

fn clone_repository(source: &Utf8Path, target: &Utf8Path) {
    let output = Command::new("git")
        .args(["clone", source.as_str(), target.as_str()])
        .output()
        .expect("spawn fixture clone");
    assert!(
        output.status.success(),
        "fixture clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct BlockingGitWorkspaceRepository {
    source: Utf8PathBuf,
    clone_started: mpsc::Sender<()>,
    release_clone: Mutex<mpsc::Receiver<()>>,
    clone_calls: AtomicUsize,
    update_calls: AtomicUsize,
    branch_repair_calls: AtomicUsize,
}

impl WorkspaceRepository for BlockingGitWorkspaceRepository {
    fn clone(&self, target: &Utf8Path) -> crate::error::Result<()> {
        self.clone_calls.fetch_add(1, Ordering::SeqCst);
        self.clone_started
            .send(())
            .expect("report clone after acquiring managed-clone lock");
        self.release_clone
            .lock()
            .expect("lock clone-release receiver")
            .recv()
            .expect("release pinned clone");
        clone_repository(&self.source, target);
        Ok(())
    }

    fn update(&self, repo: &Utf8Path) -> crate::error::Result<()> {
        self.update_calls.fetch_add(1, Ordering::SeqCst);
        crate::git::update_repository(repo)
    }

    fn ensure_default_branch(&self, repo: &Utf8Path) -> crate::error::Result<()> {
        self.branch_repair_calls.fetch_add(1, Ordering::SeqCst);
        crate::git::ensure_default_branch(repo)
    }
}

#[test]
fn pinned_and_unpinned_preparations_serialize_and_recheck_state() {
    let temp = TempDir::new().expect("create temporary workflow directory");
    let root = Utf8PathBuf::try_from(temp.path().to_owned()).expect("temporary path is UTF-8");
    let directory = Dir::open_ambient_dir(&root, ambient_authority())
        .expect("open temporary workflow capability");
    directory
        .create_dir_all("source/caller/data")
        .expect("create workflow fixture directories");
    let source = root.join("source");
    let caller = root.join("caller");
    let managed_clone = root.join("caller/data/whitaker");
    git(&source, &["init", "-b", "main"]);
    let pinned_commit = commit_file(&source, "first", "initial commit");
    git(&source, &["tag", "v1"]);
    let updated_commit = commit_file(&source, "second", "remote update");

    let dirs = ManagedCloneDirs {
        clone_dir: managed_clone.clone().into_std_path_buf(),
    };
    let (clone_started_sender, clone_started_receiver) = mpsc::channel();
    let (release_clone_sender, release_clone_receiver) = mpsc::channel();
    let repository = Arc::new(BlockingGitWorkspaceRepository {
        source: source.clone(),
        clone_started: clone_started_sender,
        release_clone: Mutex::new(release_clone_receiver),
        clone_calls: AtomicUsize::new(0),
        update_calls: AtomicUsize::new(0),
        branch_repair_calls: AtomicUsize::new(0),
    });

    let pinned = {
        let caller = caller.clone();
        let dirs = dirs.clone();
        let repository = Arc::clone(&repository);
        thread::spawn(move || {
            let preparation = WorkspacePreparation {
                dirs: &dirs,
                update: true,
                git_ref: Some("v1"),
            };
            ensure_workspace_from(&caller, &preparation, &*repository)
        })
    };
    clone_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("pinned preparation enters clone after acquiring the lock");

    let (unpinned_started_sender, unpinned_started_receiver) = mpsc::channel();
    let (unpinned_result_sender, unpinned_result_receiver) = mpsc::channel();
    let unpinned = {
        let caller = caller.clone();
        let dirs = dirs.clone();
        let repository = Arc::clone(&repository);
        thread::spawn(move || {
            unpinned_started_sender
                .send(())
                .expect("report unpinned preparation start");
            let preparation = WorkspacePreparation {
                dirs: &dirs,
                update: true,
                git_ref: None,
            };
            unpinned_result_sender.send(ensure_workspace_from(&caller, &preparation, &*repository))
        })
    };
    unpinned_started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("unpinned preparation begins while the pinned clone holds the lock");
    assert!(
        unpinned_result_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "unpinned preparation must wait for the managed-clone lock"
    );
    assert_eq!(repository.clone_calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.update_calls.load(Ordering::SeqCst), 0);
    assert_eq!(repository.branch_repair_calls.load(Ordering::SeqCst), 0);

    release_clone_sender.send(()).expect("release pinned clone");

    let pinned = pinned
        .join()
        .expect("pinned preparation thread should not panic")
        .expect("pinned preparation should succeed");
    let unpinned_checkout = unpinned_result_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("unpinned preparation completes after lock release")
        .expect("unpinned preparation should succeed");
    unpinned
        .join()
        .expect("unpinned preparation thread should not panic")
        .expect("unpinned preparation result should be delivered");

    assert_eq!(
        pinned.action,
        WorkspaceAction::CloneTo(managed_clone.clone())
    );
    assert_eq!(
        pinned.pinned_commit.as_ref().map(|commit| commit.as_str()),
        Some(pinned_commit.as_str())
    );
    assert_eq!(
        unpinned_checkout.action,
        WorkspaceAction::UpdateAt(managed_clone.clone())
    );
    assert_eq!(unpinned_checkout.pinned_commit, None);
    assert_eq!(
        crate::git::resolve_commit(&managed_clone, "refs/whitaker/pinned-ref")
            .expect("fetch stores pinned ref")
            .as_str(),
        pinned_commit
    );
    assert_eq!(
        git(&managed_clone, &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(git(&managed_clone, &["rev-parse", "HEAD"]), updated_commit);
    assert_eq!(repository.clone_calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.update_calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.branch_repair_calls.load(Ordering::SeqCst), 1);
}
