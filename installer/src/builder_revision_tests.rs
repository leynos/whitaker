//! Tests for embedding the source checkout's suite revision.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
// The only `#[rstest]` case table here drives a `sh` child, so it is `unix`
// only and this import must be gated with it. An unconditional import is
// unused on Windows, which denies warnings.
#[cfg(unix)]
use rstest::rstest;
use tempfile::TempDir;

#[cfg(unix)]
use super::apply_suite_revision;
use super::{GitSuiteRevisionProbe, SuiteRevisionProbe, format_suite_revision};
use crate::git::clear_repository_selectors;

/// Probe the fixture's checkout with the production Git probe.
///
/// A checkout the fixture controls must not produce an environment failure, so
/// one that does is a fixture bug rather than an expected probe outcome.
fn probe(fixture: &RepoFixture) -> Option<String> {
    GitSuiteRevisionProbe
        .probe(fixture.root())
        .expect("the fixture checkout should not report an environment failure")
}

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
    assert_eq!(probe(&fixture), None);
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
    assert_eq!(probe(&fixture), Some(expected.to_owned()));

    fixture.write("tracked.txt", "second");
    assert_eq!(probe(&fixture), Some(format!("{expected}+dirty")));
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
    assert_eq!(probe(&fixture), Some(expected.to_owned()));

    fixture.write("untracked.rs", "fn main() {}");
    assert_eq!(
        probe(&fixture),
        Some(format!("{expected}+dirty")),
        "an untracked source file is uncommitted source"
    );
}

/// Keep an inherited `GIT_DIR` from redirecting the probe.
///
/// Regression: the probe cleared no repository selectors, so an exported
/// `GIT_DIR` resolved the revision against another repository entirely.
#[test]
fn an_inherited_git_dir_cannot_redirect_the_probe() {
    assert_selector_cannot_change_the_probe(Inherited {
        test_name: "an_inherited_git_dir_cannot_redirect_the_probe",
        selector: "GIT_DIR",
        decoy_path: ".git",
    });
}

/// Keep an inherited index from making a clean checkout look modified.
///
/// `git status` compares the worktree against `GIT_INDEX_FILE`, so an exported
/// one made the probe add a `+dirty` suffix the source did not warrant.
#[test]
fn an_inherited_index_file_cannot_fabricate_dirt() {
    assert_selector_cannot_change_the_probe(Inherited {
        test_name: "an_inherited_index_file_cannot_fabricate_dirt",
        selector: "GIT_INDEX_FILE",
        decoy_path: ".git/index",
    });
}

/// What an inherited-selector regression needs to identify itself.
///
/// `run_in_child` re-executes one named test, so each test passes its own
/// libtest name rather than a shared helper deriving it.
#[derive(Clone, Copy)]
struct Inherited<'a> {
    /// The test's own libtest name, which the child filter matches.
    test_name: &'a str,
    /// The repository-selection variable the child inherits.
    selector: &'a str,
    /// The decoy value that makes a redirect observable.
    decoy_path: &'a str,
}

/// Run the probe in a child that inherits `case.selector` and check the result.
///
/// The probe runs in a child because the point is what an inherited environment
/// does to it, and the fixture is built there so the parent's own environment is
/// untouched.
fn assert_selector_cannot_change_the_probe(case: Inherited<'_>) {
    let Inherited {
        test_name,
        selector,
        decoy_path,
    } = case;

    // The decoy lives in the parent so both processes agree on the path the
    // child inherits; the fixture it redirects away from is the child's own.
    let decoy = RepoFixture::repository();
    decoy.write("decoy.txt", "decoy");
    decoy.git(&["add", "decoy.txt"]);
    decoy.commit();
    let decoy_value = decoy.root().join(decoy_path);

    let child_name = format!("builder::revision::tests::{test_name}");
    crate::test_utils::run_in_child(&child_name, &[(selector, decoy_value.as_str())], || {
        let (fixture, expected, _decoy) = clean_fixture_and_decoy();
        assert_eq!(
            probe(&fixture),
            Some(expected),
            "{selector} must not change what the probe reports for the checkout being built"
        );
    })
    .expect("the child test passes");
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

/// Publish a probed revision, or clear one the child would inherit.
///
/// The parent copies `inherited-sha` into the child unless
/// `apply_suite_revision` removes it, so the assertion observes the child's
/// environment rather than the test process's.
#[cfg(unix)]
#[rstest]
#[case::probed(Some("abc1234"), "abc1234")]
#[case::unprobed(None, "<unset>")]
fn a_probed_revision_replaces_an_inherited_one(
    #[case] probed: Option<&str>,
    #[case] expected: &str,
) {
    let mut command = Command::new("sh");
    command.args(["-c", "printf %s \"${WHITAKER_SUITE_REVISION-<unset>}\""]);
    command.env("WHITAKER_SUITE_REVISION", "inherited-sha");
    apply_suite_revision(&mut command, probed);

    let output = command.output().expect("the shell should run");
    let seen = String::from_utf8(output.stdout).expect("the shell output should be UTF-8");
    assert_eq!(
        seen, expected,
        "the child must see the probed revision or none at all, never the inherited one"
    );
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
    let survivors: Vec<&str> = crate::git::REPOSITORY_SELECTION_VARIABLES
        .into_iter()
        .filter(|name| {
            // Ask a shell what it inherited: the helper drains the selectors
            // from a child command, so a value surviving means the drain ran on
            // a different command than the probe builds.
            let mut command = Command::new("sh");
            command.args(["-c", &format!("printf %s \"${{{name}-<unset>}}\"")]);
            command.env(name, "inherited");
            clear_repository_selectors(&mut command);

            let output = command.output().expect("the shell should run");
            String::from_utf8(output.stdout).expect("the shell output should be UTF-8") != "<unset>"
        })
        .collect();

    assert!(
        survivors.is_empty(),
        "a probe would inherit {survivors:?} and report another repository"
    );
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
