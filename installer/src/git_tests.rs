//! Real-Git regression tests for managed-clone operations.

use std::process::{Command, Stdio};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

use super::*;
use crate::test_support::{EnvTestGuard, env_test_guard};

struct GitFixture {
    /// Serializes the test with those that change `PATH` or `HOME`, because
    /// every Git subprocess here inherits the process environment.
    _env: EnvTestGuard,
    _temp: TempDir,
    origin: Utf8PathBuf,
    clone: Utf8PathBuf,
}

fn git(args: &[&str], cwd: &Utf8Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd.as_std_path())
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(args: &[&str], cwd: &Utf8Path) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd.as_std_path())
        .stderr(Stdio::inherit())
        .output()
        .expect("git command should run");
    assert!(output.status.success(), "git {:?} should succeed", args);
    String::from_utf8(output.stdout)
        .expect("git output should be UTF-8")
        .trim()
        .to_owned()
}

fn git_fixture(default_branch: &str) -> GitFixture {
    let env = env_test_guard();
    let temp = TempDir::new().expect("temporary directory should be created");
    let root =
        Utf8PathBuf::try_from(temp.path().to_owned()).expect("temporary path should be UTF-8");
    let origin = root.join("origin");
    let clone = root.join("clone");
    let root_dir = Dir::open_ambient_dir(&root, ambient_authority())
        .expect("temporary root should be accessible");
    root_dir
        .create_dir("origin")
        .expect("origin directory should be created");

    git(&["init", "-q", "-b", default_branch, "."], &origin);
    git(&["config", "user.email", "test@example.invalid"], &origin);
    git(&["config", "user.name", "Test"], &origin);
    let origin_dir =
        Dir::open_ambient_dir(&origin, ambient_authority()).expect("origin should be accessible");
    origin_dir
        .write("suite.txt", "one")
        .expect("fixture source should be written");
    git(&["add", "suite.txt"], &origin);
    git(&["commit", "-qm", "initial"], &origin);
    git(&["clone", "-q", origin.as_str(), clone.as_str()], &root);

    GitFixture {
        _env: env,
        _temp: temp,
        origin,
        clone,
    }
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
fn update_repository_overwrites_untracked_files_that_upstream_now_tracks() {
    // A cached clone restored from an older run can carry untracked files that
    // the suite has since started to track. `git pull` aborts on them with
    // "untracked working tree files would be overwritten by merge".
    let fixture = git_fixture("main");
    let origin_dir = Dir::open_ambient_dir(&fixture.origin, ambient_authority())
        .expect("origin should be accessible");
    origin_dir
        .write("added.toml", "tracked upstream")
        .expect("new upstream file should be written");
    git(&["add", "added.toml"], &fixture.origin);
    git(&["commit", "-qm", "track a new file"], &fixture.origin);
    let clone_dir = Dir::open_ambient_dir(&fixture.clone, ambient_authority())
        .expect("clone should be accessible");
    clone_dir
        .write("added.toml", "stale untracked copy")
        .expect("stale untracked file should be written");

    update_repository(&fixture.clone).expect("update should replace the untracked file");

    assert_eq!(
        clone_dir
            .read_to_string("added.toml")
            .expect("updated file should be readable"),
        "tracked upstream"
    );
    assert_eq!(
        git_stdout(&["rev-parse", "HEAD"], &fixture.clone),
        git_stdout(&["rev-parse", "HEAD"], &fixture.origin)
    );
}

#[test]
fn update_repository_discards_local_edits_and_follows_the_default_branch() {
    // The managed clone is a build input, so a stale edit to a tracked file or a
    // default branch whose name contains a slash must not stop the update.
    let fixture = git_fixture("release/stable");
    let origin_dir = Dir::open_ambient_dir(&fixture.origin, ambient_authority())
        .expect("origin should be accessible");
    origin_dir
        .write("suite.txt", "two")
        .expect("updated fixture source should be written");
    git(&["commit", "-qam", "advance origin"], &fixture.origin);
    let clone_dir = Dir::open_ambient_dir(&fixture.clone, ambient_authority())
        .expect("clone should be accessible");
    clone_dir
        .write("suite.txt", "local edit")
        .expect("local edit should be written");

    update_repository(&fixture.clone).expect("update should discard the edit");

    assert_eq!(
        clone_dir
            .read_to_string("suite.txt")
            .expect("updated file should be readable"),
        "two"
    );
    assert_eq!(
        git_stdout(&["symbolic-ref", "--short", "HEAD"], &fixture.clone),
        "release/stable"
    );
}

#[test]
fn update_repository_follows_a_renamed_default_branch() {
    // A fetch never retargets origin/HEAD, so a clone made before the remote
    // renamed its default branch would keep resetting to the old name.
    let fixture = git_fixture("main");
    git(&["branch", "-m", "main", "trunk"], &fixture.origin);
    let origin_dir = Dir::open_ambient_dir(&fixture.origin, ambient_authority())
        .expect("origin should be accessible");
    origin_dir
        .write("suite.txt", "after the rename")
        .expect("updated fixture source should be written");
    git(&["commit", "-qam", "advance trunk"], &fixture.origin);

    update_repository(&fixture.clone).expect("update should follow the rename");

    let clone_dir = Dir::open_ambient_dir(&fixture.clone, ambient_authority())
        .expect("clone should be accessible");
    assert_eq!(
        clone_dir
            .read_to_string("suite.txt")
            .expect("updated file should be readable"),
        "after the rename"
    );
    assert_eq!(
        git_stdout(&["rev-parse", "HEAD"], &fixture.clone),
        git_stdout(&["rev-parse", "trunk"], &fixture.origin)
    );
}

fn expect_git_failure(result: Result<()>, expected_operation: &str) -> String {
    match result {
        Err(InstallerError::Git { operation, message }) => {
            assert_eq!(operation, expected_operation);
            assert!(!message.is_empty(), "Git's diagnostic should be carried");
            message
        }
        other => panic!("expected a {expected_operation} failure, got {other:?}"),
    }
}

#[test]
fn update_repository_names_the_fetch_when_the_remote_is_unreachable() {
    let fixture = git_fixture("main");
    git(
        &[
            "remote",
            "set-url",
            "origin",
            "/nonexistent/whitaker-origin",
        ],
        &fixture.clone,
    );

    let message = expect_git_failure(update_repository(&fixture.clone), "fetch");

    assert!(message.contains("nonexistent"), "diagnostic: {message}");
}

#[test]
fn update_repository_names_set_head_when_the_remote_has_no_default_branch() {
    // The remote's HEAD names a branch that does not exist, so the default
    // branch cannot be determined.
    let fixture = git_fixture("main");
    git(
        &["symbolic-ref", "HEAD", "refs/heads/nonexistent"],
        &fixture.origin,
    );

    let message = expect_git_failure(update_repository(&fixture.clone), "set-head");

    assert!(message.contains("HEAD"), "diagnostic: {message}");
}

#[test]
fn update_repository_names_the_reset_when_the_index_is_locked() {
    let fixture = git_fixture("main");
    let clone_dir = Dir::open_ambient_dir(&fixture.clone, ambient_authority())
        .expect("clone should be accessible");
    clone_dir
        .write(".git/index.lock", "")
        .expect("lock file should be written");

    let message = expect_git_failure(update_repository(&fixture.clone), "reset");

    assert!(message.contains("index.lock"), "diagnostic: {message}");
}

#[test]
fn checkout_ref_prefers_the_current_remote_tracking_branch() {
    let fixture = git_fixture("main");
    let origin_dir = Dir::open_ambient_dir(&fixture.origin, ambient_authority())
        .expect("origin should be accessible");
    origin_dir
        .write("suite.txt", "two")
        .expect("updated fixture source should be written");
    git(&["commit", "-am", "advance origin"], &fixture.origin);
    git(&["fetch", "origin"], &fixture.clone);

    let stale_local = git_stdout(&["rev-parse", "main"], &fixture.clone);
    let remote_tip = git_stdout(&["rev-parse", "origin/main"], &fixture.clone);
    assert_ne!(stale_local, remote_tip, "local branch should remain stale");

    let reference = "main".try_into().expect("valid suite reference");
    checkout_ref(&fixture.clone, &reference).expect("checkout should succeed");

    assert_eq!(
        git_stdout(&["rev-parse", "HEAD"], &fixture.clone),
        remote_tip
    );
    assert!(
        is_detached_head(&fixture.clone).expect("HEAD should be readable"),
        "the suite checkout should be detached"
    );
}

#[test]
fn restore_default_branch_preserves_a_slash_in_its_name() {
    let fixture = git_fixture("release/stable");
    git(&["checkout", "--detach"], &fixture.clone);

    restore_default_branch(&fixture.clone).expect("default branch should be restored");

    assert_eq!(
        git_stdout(&["symbolic-ref", "--short", "HEAD"], &fixture.clone),
        "release/stable"
    );
}
