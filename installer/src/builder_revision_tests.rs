//! Tests for embedding the source checkout's suite revision.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use tempfile::TempDir;

#[cfg(unix)]
use super::apply_suite_revision;
use super::{format_suite_revision, suite_revision};
use crate::git::clear_repository_selectors;

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

/// Keep an inherited repository selector from changing what the probe reports.
///
/// Regression: the probe cleared no repository selectors. An exported `GIT_DIR`
/// resolved the revision against another repository, and an exported
/// `GIT_INDEX_FILE` made `git status` compare the worktree against another
/// repository's tracked set, adding a `+dirty` suffix the source did not
/// warrant.
#[rstest]
#[case::git_dir("GIT_DIR", ".git")]
#[case::index_file("GIT_INDEX_FILE", ".git/index")]
fn an_inherited_selector_cannot_change_the_probe(#[case] selector: &str, #[case] decoy_path: &str) {
    let (fixture, expected, decoy) = clean_fixture_and_decoy();
    let decoy_value = decoy.root().join(decoy_path);

    let _guard = whitaker_common::test_support::env_test_guard();
    let probed = temp_env::with_var(selector, Some(decoy_value.as_str()), || {
        suite_revision(fixture.root())
    });

    assert_eq!(
        probed,
        Some(expected),
        "{selector} must not change what the probe reports for the checkout being built"
    );
}

/// Build a clean fixture and a decoy repository with a different `HEAD`.
///
/// The decoy makes a redirected probe detectable; the assertion it differs
/// keeps each case from passing vacuously.
fn clean_fixture_and_decoy() -> (RepoFixture, String, RepoFixture) {
    let fixture = RepoFixture::repository();
    fixture.write("tracked.txt", "first");
    fixture.git(&["add", "tracked.txt"]);
    fixture.commit();

    let decoy = RepoFixture::repository();
    decoy.write("decoy.txt", "decoy");
    decoy.git(&["add", "decoy.txt"]);
    decoy.commit();

    let head = fixture.git(&["rev-parse", "--short", "HEAD"]);
    let decoy_head = decoy.git(&["rev-parse", "--short", "HEAD"]);
    assert_ne!(
        head.trim(),
        decoy_head.trim(),
        "the decoy repository must differ from the fixture for these tests to mean anything"
    );
    (fixture, head.trim().to_owned(), decoy)
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

/// Keep every repository selector out of the probe's child environment.
///
/// An exported selector outranks `-C`, so a caller that exports one would
/// otherwise have the revision probe report a different repository. The test
/// process environment is not mutated: each selector is set on the child
/// command and then drained by the production helper, which is the same path a
/// probe takes.
#[cfg(unix)]
#[test]
fn a_probe_never_inherits_a_repository_selector() {
    let inherited: Vec<&str> = crate::git::REPOSITORY_SELECTION_VARIABLES
        .into_iter()
        .filter(|name| child_selector(name).is_some())
        .collect();

    assert!(
        inherited.is_empty(),
        "a probe would inherit {inherited:?} and report another repository"
    );
}

/// Run a shell that reports `name` from the environment it inherited.
#[cfg(unix)]
fn child_selector(name: &str) -> Option<String> {
    let mut command = Command::new("sh");
    command.args(["-c", &format!("printf %s \"${{{name}-<unset>}}\"")]);
    command.env(name, "inherited");
    clear_repository_selectors(&mut command);

    let output = command.output().expect("the shell should run");
    let value = String::from_utf8(output.stdout).expect("the shell output should be UTF-8");
    (value != "<unset>").then_some(value)
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
    /// `-C` alone cannot confine the command to the temporary directory,
    /// because the repository selectors override discovery afterwards. Clearing
    /// them with the production helper keeps `add` and `commit` inside the
    /// fixture rather than in whatever repository the test process inherited.
    fn git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        command.arg("-C").arg(self.root.as_str()).args(args);
        clear_repository_selectors(&mut command);

        let output = command.output().expect("git should run");
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
