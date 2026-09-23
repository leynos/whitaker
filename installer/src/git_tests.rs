//! Real-Git regression tests for managed-clone operations.

use super::*;
use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::process::{Command, Stdio};
use tempfile::TempDir;

struct GitFixture {
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
fn update_repository_error_includes_operation() {
    let err = InstallerError::Git {
        operation: "pull",
        message: "not a git repository".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("pull"));
    assert!(msg.contains("not a git repository"));
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
