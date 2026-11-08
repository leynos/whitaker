//! The Git adapter keeps infrastructure failure distinct from bad evidence.
//!
//! A Git that cannot start says nothing about the snapshot, so it must reach
//! the caller as [`ClippySnapshotError::Git`] rather than as a rejection of
//! the repository. The complementary case, a clone that ran and failed, is
//! covered end to end by `rejects_corrupt_bundles::case_4_truncated_bundle`.

use camino::Utf8Path;

use super::{
    super::{error::ClippySnapshotError, ports::BundleOperations},
    SnapshotGit,
};
use crate::test_utils::SnapshotFixture;

#[test]
fn a_git_that_cannot_start_is_an_infrastructure_failure() {
    // A working directory that does not exist makes the spawn itself fail, so
    // no Git process ever runs and no snapshot evidence is produced.
    let missing = Utf8Path::new("/nonexistent-whitaker-snapshot/git-working-directory");

    let error = SnapshotGit::local()
        .stdout(
            "rev-parse",
            Some(missing),
            &["rev-parse", "--is-bare-repository"],
        )
        .expect_err("Git cannot start in a directory that does not exist");

    assert!(
        matches!(&error, ClippySnapshotError::Git { operation, .. } if *operation == "rev-parse"),
        "{error}"
    );
}

/// The clone path reports a Git that cannot start as an infrastructure fault.
///
/// Unix only: an empty `PATH` in the child's environment hides `git` from
/// `execvp`, but Windows resolves the executable against the parent's search
/// path, so the same trick still starts Git there and reports the missing
/// bundle instead. The working-directory case above covers spawn failure on
/// both platforms.
#[cfg(unix)]
#[test]
fn a_clone_that_cannot_start_is_an_infrastructure_failure() {
    let envs = [("PATH", "")];

    let error = SnapshotGit::local()
        .with_env(&envs)
        .stage_clone(
            Utf8Path::new("/snapshot.bundle"),
            Utf8Path::new("/snapshot.git"),
        )
        .expect_err("Git cannot start without a PATH");

    assert!(
        matches!(&error, ClippySnapshotError::Git { operation, .. } if *operation == "clone"),
        "{error}"
    );
}

#[test]
fn git_boolean_answers_are_interpreted_strictly() {
    assert_eq!(
        super::boolean_answer("rev-parse", "shallow repository", "true"),
        Ok(true)
    );
    assert_eq!(
        super::boolean_answer("rev-parse", "shallow repository", "false"),
        Ok(false)
    );
}

#[test]
fn an_unexpected_answer_to_a_boolean_question_is_an_infrastructure_failure() {
    // Accepting any other output as "not shallow" would admit history whose
    // depth was never established, so it must not become evidence.
    let error = super::boolean_answer("rev-parse", "shallow repository", "maybe")
        .expect_err("only `true` and `false` answer a Git boolean question");

    assert!(
        matches!(&error, ClippySnapshotError::Git { operation, .. } if *operation == "rev-parse"),
        "{error}"
    );
}

#[test]
fn listing_heads_leaves_the_bundle_and_its_directory_untouched() {
    // `git bundle list-heads` needs a repository, so the adapter makes a
    // scratch one. The port promises an observation, so the scratch state must
    // never appear beside the bundle the caller owns.
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let bundle = fixture.bundle();
    let directory = bundle.parent().expect("the bundle has a parent");
    let entries_before = directory_entries(directory);
    let bytes_before = std::fs::read(bundle).expect("read the bundle");

    let heads = SnapshotGit::local()
        .list_heads(bundle)
        .expect("list the bundle's heads");

    assert!(
        heads.iter().any(|head| head.object == fixture.commit()),
        "{heads:?}"
    );
    assert_eq!(
        directory_entries(directory),
        entries_before,
        "the query must not leave scratch state beside the bundle"
    );
    assert_eq!(
        std::fs::read(bundle).expect("read the bundle"),
        bytes_before,
        "the query must not rewrite the bundle"
    );
}

#[test]
fn listing_heads_creates_its_scratch_repository_under_the_supplied_parent() {
    // A parent that a directory cannot be made in fails before Git runs, so
    // only a scratch location the adapter actually uses can produce this error.
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let parent = holder.path().join("occupied");
    std::fs::write(&parent, "not a directory").expect("occupy the scratch parent");
    let parent = Utf8Path::from_path(&parent).expect("the scratch parent is UTF-8");

    let error = SnapshotGit::local()
        .with_scratch_parent(parent)
        .list_heads(fixture.bundle())
        .expect_err("a file cannot hold the scratch repository");

    assert!(matches!(&error, ClippySnapshotError::Io { .. }), "{error}");
}

#[test]
fn listing_heads_removes_its_scratch_repository() {
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let parent = tempfile::tempdir().expect("create a temporary directory");
    let parent = Utf8Path::from_path(parent.path()).expect("the scratch parent is UTF-8");

    SnapshotGit::local()
        .with_scratch_parent(parent)
        .list_heads(fixture.bundle())
        .expect("list the bundle's heads");

    assert_eq!(
        directory_entries(parent),
        Vec::<String>::new(),
        "the scratch repository must not outlive the query"
    );
}

#[test]
fn a_failed_listing_still_removes_its_scratch_repository() {
    let parent = tempfile::tempdir().expect("create a temporary directory");
    let parent = Utf8Path::from_path(parent.path()).expect("the scratch parent is UTF-8");
    let missing = parent.join("missing.bundle");

    let error = SnapshotGit::local()
        .with_scratch_parent(parent)
        .list_heads(&missing)
        .expect_err("an absent bundle cannot list heads");

    assert!(matches!(&error, ClippySnapshotError::Git { .. }), "{error}");
    assert_eq!(
        directory_entries(parent),
        Vec::<String>::new(),
        "the scratch repository must be removed after a failure too"
    );
}

/// Return the sorted entry names of `directory`.
fn directory_entries(directory: &Utf8Path) -> Vec<String> {
    let mut entries: Vec<String> = std::fs::read_dir(directory)
        .expect("read the bundle's directory")
        .map(|entry| {
            entry
                .expect("read a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    entries.sort();
    entries
}
