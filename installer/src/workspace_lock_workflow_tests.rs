//! Real-Git workflow coverage for managed-clone lock serialization.

use super::super::{
    GitWorkspaceRepository, ManagedCloneLock, WorkspaceAction, WorkspacePreparation,
    ensure_workspace_from,
};
use crate::dirs::BaseDirs;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
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

#[test]
fn managed_clone_waiter_rechecks_state_and_updates_after_lock_release() {
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
    commit_file(&source, "first", "initial commit");

    let lock = ManagedCloneLock::acquire(&managed_clone).expect("acquire managed-clone lock");
    let dirs = ManagedCloneDirs {
        clone_dir: managed_clone.clone().into_std_path_buf(),
    };
    let (started_sender, started_receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let waiter = thread::spawn(move || {
        started_sender.send(()).expect("report waiter start");
        let preparation = WorkspacePreparation {
            dirs: &dirs,
            update: true,
            git_ref: None,
        };
        result_sender.send(ensure_workspace_from(
            &caller,
            &preparation,
            &GitWorkspaceRepository,
        ))
    });
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("waiter begins workspace preparation");
    assert!(
        result_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "workspace preparation must wait for the managed-clone lock"
    );

    clone_repository(&source, &managed_clone);
    let expected_commit = commit_file(&source, "second", "remote update");
    drop(lock);

    let checkout = result_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("waiter completes after lock release")
        .expect("waiter workspace preparation succeeds");
    waiter
        .join()
        .expect("waiter thread should not panic")
        .expect("waiter reports its workspace result");

    assert_eq!(
        checkout.action,
        WorkspaceAction::UpdateAt(managed_clone.clone())
    );
    assert_eq!(
        git(&managed_clone, &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(git(&managed_clone, &["rev-parse", "HEAD"]), expected_commit);
}
