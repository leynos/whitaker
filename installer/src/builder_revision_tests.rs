//! Tests for embedding the source checkout's suite revision.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

#[cfg(unix)]
use super::apply_suite_revision;
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
    let fixture = RepoFixture::empty();
    assert_eq!(suite_revision(fixture.root()), None);
}

/// Embed `HEAD` and mark a modified worktree.
#[test]
fn head_revision_is_embedded_and_dirty_worktrees_are_marked() {
    let fixture = RepoFixture::repository();
    fixture.write("tracked.txt", "first");
    fixture.git(&["add", "tracked.txt"]);
    fixture.commit();

    let head = fixture.git(&["rev-parse", "--short", "HEAD"]);
    let expected = head.trim();
    assert_eq!(suite_revision(fixture.root()), Some(expected.to_owned()));

    fixture.write("tracked.txt", "second");
    assert_eq!(
        suite_revision(fixture.root()),
        Some(format!("{expected}+dirty"))
    );
}

/// Treat an untracked source file as dirty source, whatever Git is configured
/// to report.
///
/// A checkout whose only change is a new module compiles uncommitted source, so
/// the revision must say so. `status.showUntrackedFiles=no` would otherwise
/// hide that file from the porcelain probe and leave the revision looking clean.
#[test]
fn untracked_source_is_reported_despite_git_status_configuration() {
    let fixture = RepoFixture::repository();
    fixture.git(&["config", "status.showUntrackedFiles", "no"]);
    fixture.write("tracked.txt", "first");
    fixture.git(&["add", "tracked.txt"]);
    fixture.commit();

    let head = fixture.git(&["rev-parse", "--short", "HEAD"]);
    let expected = head.trim();
    assert_eq!(suite_revision(fixture.root()), Some(expected.to_owned()));

    fixture.write("untracked.rs", "fn main() {}");
    assert_eq!(
        suite_revision(fixture.root()),
        Some(format!("{expected}+dirty")),
        "an untracked source file is uncommitted source"
    );
}

/// Publish a probed revision to the child process.
#[cfg(unix)]
#[test]
fn a_probed_revision_reaches_the_child() {
    assert_eq!(
        child_revision(Some("abc1234".to_owned())),
        "abc1234",
        "the child should see the revision this checkout reported"
    );
}

/// Clear a revision the child would otherwise inherit.
#[cfg(unix)]
#[test]
fn a_failed_probe_clears_an_inherited_revision() {
    assert_eq!(
        child_revision(None),
        "<unset>",
        "an unprobed build must not inherit another checkout's revision"
    );
}

/// Run a shell that reports the revision its environment carries.
///
/// The parent copies `inherited-sha` into the child unless
/// `apply_suite_revision` removes it, so the assertion observes the child's
/// environment rather than the test process's.
#[cfg(unix)]
fn child_revision(revision: Option<String>) -> String {
    let mut command = Command::new("sh");
    command.args(["-c", "printf %s \"${WHITAKER_SUITE_REVISION-<unset>}\""]);
    command.env("WHITAKER_SUITE_REVISION", "inherited-sha");
    apply_suite_revision(&mut command, revision);

    let output = command.output().expect("the shell should run");
    String::from_utf8(output.stdout).expect("the shell output should be UTF-8")
}

/// A temporary directory the tests write into through a capability handle.
///
/// Holding the handle keeps the fixture's filesystem access explicit, matching
/// the crate's use of `cap_std` instead of `std::fs`.
struct RepoFixture {
    _temp: TempDir,
    root: Utf8PathBuf,
    dir: Dir,
}

impl RepoFixture {
    /// Create an empty temporary directory.
    fn empty() -> Self {
        let temp = TempDir::new().expect("temporary directory should be created");
        let root =
            Utf8PathBuf::try_from(temp.path().to_owned()).expect("temporary path should be UTF-8");
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .expect("temporary directory should be accessible");
        Self {
            _temp: temp,
            root,
            dir,
        }
    }

    /// Create a temporary directory initialized as a Git repository.
    fn repository() -> Self {
        let fixture = Self::empty();
        fixture.git(&["init", "--quiet"]);
        fixture
    }

    /// Return the fixture's root path.
    fn root(&self) -> &Utf8Path { &self.root }

    /// Write `contents` to `name` inside the fixture.
    fn write(&self, name: &str, contents: &str) {
        self.dir
            .write(name, contents)
            .expect("fixture file should be written");
    }

    /// Run one Git command in the fixture, failing the test when it exits
    /// unsuccessfully.
    ///
    /// `GIT_DIR` and `GIT_WORK_TREE` override repository discovery, so `-C`
    /// alone cannot confine the command to the temporary directory. Removing
    /// them keeps `add` and `commit` inside the fixture rather than in whatever
    /// repository the test process inherited.
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.root.as_str())
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
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
    fn commit(&self) {
        self.git(&[
            "-c",
            "user.name=Whitaker Tests",
            "-c",
            "user.email=whitaker@example.invalid",
            "commit",
            // A developer with `commit.gpgSign=true` would otherwise make this
            // fixture depend on signing credentials it has no reason to use.
            "--no-gpg-sign",
            "--quiet",
            "-m",
            "initial",
        ]);
    }
}
