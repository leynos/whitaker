//! Tests for embedding the source checkout's suite revision.

use std::{fs, process::Command};

use camino::Utf8Path;
use tempfile::TempDir;

use super::{format_suite_revision, suite_revision};

/// Format an embedded revision, marking dirty source.
#[test]
fn clean_revision_is_the_short_sha() {
    assert_eq!(format_suite_revision("abc1234", false), "abc1234");
}

/// Mark a revision whose worktree has uncommitted changes.
#[test]
fn dirty_revision_carries_the_dirty_suffix() {
    assert_eq!(format_suite_revision("abc1234", true), "abc1234+dirty");
}

/// Leave the revision unset outside a Git repository.
#[test]
fn missing_repository_leaves_the_revision_unset() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let root = Utf8Path::from_path(temp.path()).expect("temporary path should be UTF-8");
    assert_eq!(suite_revision(root), None);
}

/// Embed `HEAD` and mark a modified worktree.
#[test]
fn head_revision_is_embedded_and_dirty_worktrees_are_marked() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let root = Utf8Path::from_path(temp.path()).expect("temporary path should be UTF-8");
    run_git(root, &["init", "--quiet"]);
    fs::write(root.join("tracked.txt"), "first").expect("fixture file should be written");
    run_git(root, &["add", "tracked.txt"]);
    run_git_commit(root);

    let head = run_git(root, &["rev-parse", "--short", "HEAD"]);
    let expected = head.trim();
    assert_eq!(suite_revision(root), Some(expected.to_owned()));

    fs::write(root.join("tracked.txt"), "second").expect("fixture file should be rewritten");
    assert_eq!(suite_revision(root), Some(format!("{expected}+dirty")));
}

/// Run one Git command in `root`, failing the test when it exits unsuccessfully.
fn run_git(root: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_str())
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git output should be UTF-8")
}

/// Commit the staged fixture with an identity local to the test repository.
fn run_git_commit(root: &Utf8Path) {
    run_git(
        root,
        &[
            "-c",
            "user.name=Whitaker Tests",
            "-c",
            "user.email=whitaker@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
}
