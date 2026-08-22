//! Real-Git regression tests for clone updates and pinned checkouts.

use super::*;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
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
    let directory =
        Dir::open_ambient_dir(dir, ambient_authority()).expect("open fixture directory");
    directory.write(name, contents).expect("write fixture file");
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
#[fixture]
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

#[rstest]
fn resolve_commit_resolves_tag_branch_and_sha(git_fixture: GitFixture) {
    assert_eq!(
        resolve_commit(&git_fixture.clone, "v1")
            .expect("resolve tag")
            .as_str(),
        git_fixture.first.as_str()
    );
    assert_eq!(
        resolve_commit(&git_fixture.clone, "main")
            .expect("resolve branch")
            .as_str(),
        git_fixture.second.as_str()
    );
    assert_eq!(
        resolve_commit(&git_fixture.clone, &git_fixture.second)
            .expect("resolve sha")
            .as_str(),
        git_fixture.second.as_str()
    );
}

#[rstest]
fn resolve_commit_errors_on_garbage(git_fixture: GitFixture) {
    let err =
        resolve_commit(&git_fixture.clone, "definitely-not-a-ref").expect_err("expected error");
    assert!(matches!(err, InstallerError::Git { .. }), "got {err:?}");
}

#[rstest]
fn checkout_detached_leaves_head_at_commit(git_fixture: GitFixture) {
    let commit = resolve_commit(&git_fixture.clone, "v1").expect("resolve tag");
    checkout_detached(&git_fixture.clone, &commit).expect("checkout detached");
    assert_eq!(
        git(&git_fixture.clone, &["rev-parse", "HEAD"]),
        git_fixture.first
    );
    // A detached HEAD has no symbolic ref.
    let symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(git_fixture.clone.as_std_path())
        .output()
        .expect("spawn symbolic-ref");
    assert!(!symbolic.status.success(), "expected detached HEAD");
}

#[rstest]
fn unpinned_no_update_preserves_detached_commit_gate(git_fixture: GitFixture) {
    crate::workspace::pin_to_ref(&git_fixture.clone, "v1").expect("pin initial install");

    let checkout = crate::workspace::finalize_workspace_checkout(
        git_fixture.clone.clone(),
        None,
        crate::workspace::WorkspaceAction::UseExisting(git_fixture.clone.clone()),
    )
    .expect("reuse detached checkout without updating");

    assert_eq!(checkout.pinned_commit, None);
    assert_eq!(
        checkout.detached_commit.as_ref().map(CommitSha::as_str),
        Some(git_fixture.first.as_str())
    );
    assert_eq!(
        checkout.expected_git_sha().map(CommitSha::as_str),
        Some(git_fixture.first.as_str())
    );
}

#[rstest]
fn pinned_checkout_reattaches_for_unpinned_update(git_fixture: GitFixture) {
    let pinned_commit =
        crate::workspace::pin_to_ref(&git_fixture.clone, "v1").expect("pin checkout to v1");
    assert_eq!(pinned_commit.as_str(), git_fixture.first);
    assert_eq!(
        git(&git_fixture.clone, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );

    let source = Utf8PathBuf::try_from(git_fixture._source.path().to_owned()).expect("UTF-8 path");
    let third = commit_file(&source, "c.txt", "three", "third");

    ensure_default_branch(&git_fixture.clone).expect("reattach to default branch");
    update_repository(&git_fixture.clone).expect("update after reattach");

    assert_eq!(
        git(&git_fixture.clone, &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(git(&git_fixture.clone, &["rev-parse", "HEAD"]), third);
}

#[rstest]
fn ensure_default_branch_is_noop_on_a_branch(git_fixture: GitFixture) {
    ensure_default_branch(&git_fixture.clone).expect("noop on branch");
    assert_eq!(
        git(&git_fixture.clone, &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
}

#[rstest]
fn default_branch_name_does_not_repair_missing_remote_head(git_fixture: GitFixture) {
    git(
        &git_fixture.clone,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );

    assert_eq!(
        default_branch_name(&git_fixture.clone).expect("query default branch"),
        None
    );
    let remote_head = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(git_fixture.clone.as_std_path())
        .output()
        .expect("query missing origin HEAD");
    assert!(
        !remote_head.status.success(),
        "query must not repair origin/HEAD"
    );
}

#[rstest]
fn ensure_default_branch_repairs_missing_remote_head(git_fixture: GitFixture) {
    let commit = resolve_commit(&git_fixture.clone, "v1").expect("resolve tag");
    checkout_detached(&git_fixture.clone, &commit).expect("checkout detached");
    git(
        &git_fixture.clone,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );

    ensure_default_branch(&git_fixture.clone).expect("repair and reattach default branch");

    assert_eq!(
        git(&git_fixture.clone, &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(
        git(
            &git_fixture.clone,
            &["symbolic-ref", "refs/remotes/origin/HEAD"]
        ),
        "refs/remotes/origin/main"
    );
}

#[rstest]
fn default_branch_validation_rejects_option_like_ref(git_fixture: GitFixture) {
    let err = validate_default_branch(&git_fixture.clone, "--orphan=attacker")
        .expect_err("option-like default branch must be rejected");

    assert!(
        matches!(
            err,
            InstallerError::Git {
                operation: "check-ref-format",
                ..
            }
        ),
        "got {err:?}"
    );
}

#[rstest]
fn fetch_ref_retrieves_a_new_tag(git_fixture: GitFixture) {
    // Add a third commit and tag it in the source, after the clone was made.
    let source = Utf8PathBuf::try_from(git_fixture._source.path().to_owned()).expect("UTF-8 path");
    let third = commit_file(&source, "c.txt", "three", "third");
    git(&source, &["tag", "v2"]);
    commit_file(&source, "d.txt", "four", "unrelated tag commit");
    git(&source, &["tag", "unrelated"]);

    // The clone cannot resolve the new tag until it fetches.
    assert!(resolve_commit(&git_fixture.clone, "v2").is_err());
    let fetched = fetch_ref(&git_fixture.clone, "v2").expect("fetch new tag");
    assert_eq!(fetched.as_str(), third);
    assert_eq!(
        resolve_commit(&git_fixture.clone, PINNED_REF)
            .expect("resolve private pinned ref")
            .as_str(),
        third
    );
    assert_eq!(
        resolve_commit(&git_fixture.clone, "v2")
            .expect("resolve v2")
            .as_str(),
        third
    );
    assert!(
        resolve_commit(&git_fixture.clone, "unrelated").is_err(),
        "fetching v2 must not transfer unrelated tags"
    );
}

#[rstest]
fn pin_to_ref_fetches_and_checks_out_a_new_remote_branch(git_fixture: GitFixture) {
    let source = Utf8PathBuf::try_from(git_fixture._source.path().to_owned()).expect("UTF-8 path");
    git(&source, &["checkout", "-b", "release-candidate"]);
    let branch_commit = commit_file(&source, "c.txt", "three", "branch commit");

    assert!(resolve_commit(&git_fixture.clone, "release-candidate").is_err());
    let pinned_commit = crate::workspace::pin_to_ref(&git_fixture.clone, "release-candidate")
        .expect("fetch and pin new remote branch");

    assert_eq!(pinned_commit.as_str(), branch_commit);
    assert_eq!(
        git(&git_fixture.clone, &["rev-parse", "HEAD"]),
        branch_commit
    );
    assert_eq!(
        git(&git_fixture.clone, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );
}

#[rstest]
fn pin_to_ref_prefers_an_updated_remote_branch(git_fixture: GitFixture) {
    let source = Utf8PathBuf::try_from(git_fixture._source.path().to_owned()).expect("UTF-8 path");
    let third = commit_file(&source, "c.txt", "three", "third");

    let pinned_commit = crate::workspace::pin_to_ref(&git_fixture.clone, "main")
        .expect("fetch and pin updated main");

    assert_eq!(pinned_commit.as_str(), third);
    assert_eq!(git(&git_fixture.clone, &["rev-parse", "HEAD"]), third);
}

#[rstest]
fn pin_to_ref_falls_back_to_a_local_ref_offline(git_fixture: GitFixture) {
    git(&git_fixture.clone, &["remote", "remove", "origin"]);

    let pinned_commit = crate::workspace::pin_to_ref(&git_fixture.clone, "v1")
        .expect("pin locally available tag while offline");

    assert_eq!(pinned_commit.as_str(), git_fixture.first);
    assert_eq!(
        git(&git_fixture.clone, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );
}

#[test]
fn clone_repository_error_includes_operation() {
    let target = TempDir::new().expect("clone target temp dir");
    let target_path = Utf8PathBuf::try_from(target.path().to_owned()).expect("UTF-8 target path");
    let target_dir = Dir::open_ambient_dir(&target_path, ambient_authority())
        .expect("open clone target directory");
    target_dir
        .write("occupied", b"occupied")
        .expect("write target file");

    let err = clone_repository(&target_path).expect_err("clone into non-empty target should fail");

    let InstallerError::Git { operation, message } = err else {
        panic!("expected Git error, got {err:?}");
    };
    assert_eq!(operation, "clone");
    assert!(!message.is_empty(), "expected Git stderr");
}

#[test]
fn update_repository_error_includes_operation() {
    let repo = TempDir::new().expect("non-repository temp dir");
    let repo_path = Utf8PathBuf::try_from(repo.path().to_owned()).expect("UTF-8 path");

    let err = update_repository(&repo_path).expect_err("pull outside a repository should fail");

    let InstallerError::Git { operation, message } = err else {
        panic!("expected Git error, got {err:?}");
    };
    assert_eq!(operation, "pull");
    assert!(
        message.contains("not a git repository"),
        "stderr: {message}"
    );
}
