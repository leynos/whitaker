//! Real-Git regression tests for clone updates and pinned checkouts.

use super::*;
use camino::Utf8PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Run a Git command in `dir`, asserting success, and return trimmed stdout.
fn git(dir: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir.as_std_path())
        .output()
        .expect("failed to spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Commit the given file content in `dir` and return the resulting SHA.
fn commit_file(dir: &Utf8Path, name: &str, contents: &str, message: &str) -> String {
    std::fs::write(dir.join(name).as_std_path(), contents).expect("write fixture file");
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

/// A source repository plus a clone of it, with recorded commit SHAs.
struct GitFixture {
    _source: TempDir,
    _clone: TempDir,
    clone: Utf8PathBuf,
    first: String,
    second: String,
}

/// Build a source repo (two commits, tag `v1` on the first) and clone it.
fn git_fixture() -> GitFixture {
    let source = TempDir::new().expect("source temp dir");
    let source_path = Utf8PathBuf::try_from(source.path().to_owned()).expect("UTF-8 source path");
    git(&source_path, &["init", "-b", "main"]);
    let first = commit_file(&source_path, "a.txt", "one", "first");
    git(&source_path, &["tag", "v1"]);
    let second = commit_file(&source_path, "b.txt", "two", "second");

    let clone = TempDir::new().expect("clone temp dir");
    let clone_path = Utf8PathBuf::try_from(clone.path().to_owned()).expect("UTF-8 clone path");
    let output = Command::new("git")
        .args(["clone", source_path.as_str(), clone_path.as_str()])
        .output()
        .expect("failed to spawn git clone");
    assert!(
        output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    GitFixture {
        _source: source,
        _clone: clone,
        clone: clone_path,
        first,
        second,
    }
}

#[test]
fn resolve_commit_resolves_tag_branch_and_sha() {
    let fx = git_fixture();
    assert_eq!(
        resolve_commit(&fx.clone, "v1").expect("resolve tag"),
        fx.first
    );
    assert_eq!(
        resolve_commit(&fx.clone, "main").expect("resolve branch"),
        fx.second
    );
    assert_eq!(
        resolve_commit(&fx.clone, &fx.second).expect("resolve sha"),
        fx.second
    );
}

#[test]
fn resolve_commit_errors_on_garbage() {
    let fx = git_fixture();
    let err = resolve_commit(&fx.clone, "definitely-not-a-ref").expect_err("expected error");
    assert!(matches!(err, InstallerError::Git { .. }), "got {err:?}");
}

#[test]
fn checkout_detached_leaves_head_at_commit() {
    let fx = git_fixture();
    checkout_detached(&fx.clone, &fx.first).expect("checkout detached");
    assert_eq!(git(&fx.clone, &["rev-parse", "HEAD"]), fx.first);
    // A detached HEAD has no symbolic ref.
    let symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(fx.clone.as_std_path())
        .output()
        .expect("spawn symbolic-ref");
    assert!(!symbolic.status.success(), "expected detached HEAD");
}

#[test]
fn ensure_default_branch_reattaches_so_update_succeeds() {
    let fx = git_fixture();
    checkout_detached(&fx.clone, &fx.first).expect("checkout detached");
    ensure_default_branch(&fx.clone).expect("reattach to default branch");
    assert_eq!(git(&fx.clone, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    // A pull now succeeds because HEAD is on a branch again.
    update_repository(&fx.clone).expect("update after reattach");
}

#[test]
fn ensure_default_branch_is_noop_on_a_branch() {
    let fx = git_fixture();
    ensure_default_branch(&fx.clone).expect("noop on branch");
    assert_eq!(git(&fx.clone, &["symbolic-ref", "HEAD"]), "refs/heads/main");
}

#[test]
fn fetch_ref_retrieves_a_new_tag() {
    let fx = git_fixture();
    // Add a third commit and tag it in the source, after the clone was made.
    let source = Utf8PathBuf::try_from(fx._source.path().to_owned()).expect("UTF-8 path");
    let third = commit_file(&source, "c.txt", "three", "third");
    git(&source, &["tag", "v2"]);

    // The clone cannot resolve the new tag until it fetches.
    assert!(resolve_commit(&fx.clone, "v2").is_err());
    fetch_ref(&fx.clone, "v2").expect("fetch new tag");
    assert_eq!(resolve_commit(&fx.clone, "v2").expect("resolve v2"), third);
}

#[test]
fn clone_repository_error_includes_operation() {
    let err = InstallerError::Git {
        operation: "clone",
        message: "test error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("clone"));
    assert!(msg.contains("test error"));
}

#[test]
fn update_repository_error_includes_operation() {
    let err = InstallerError::Git {
        operation: "pull",
        message: "not a git repository".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("pull"));
    assert!(msg.contains("not a git repository"));
}
