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
    // `git bundle list-heads` reads only the bundle header and needs no
    // repository, so the port's read-only promise holds literally: nothing
    // beside the bundle changes.
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
        "the query must not leave state beside the bundle"
    );
    assert_eq!(
        std::fs::read(bundle).expect("read the bundle"),
        bytes_before,
        "the query must not rewrite the bundle"
    );
}

#[test]
fn listing_heads_creates_no_scratch_repository() {
    // A regular file cannot hold a scratch directory, so a listing that still
    // needed one would fail here. It succeeds, and the file is untouched, only
    // because the query writes nothing.
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let occupied = holder.path().join("occupied");
    std::fs::write(&occupied, "not a directory").expect("occupy the scratch parent");
    let scratch_parent = Utf8Path::from_path(&occupied).expect("the scratch parent is UTF-8");

    let heads = SnapshotGit::local()
        .with_scratch_parent(scratch_parent)
        .list_heads(fixture.bundle())
        .expect("listing heads needs no scratch repository");

    assert!(!heads.is_empty(), "{heads:?}");
    assert_eq!(
        std::fs::read_to_string(&occupied).expect("read the occupied parent"),
        "not a directory",
        "the query must not touch the scratch parent"
    );
}

#[test]
fn listing_heads_works_outside_a_repository() {
    // `git bundle list-heads` needs no repository. Running it from a directory
    // that is not one proves the query does not depend on the process's
    // working directory happening to sit inside a checkout.
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let outside_dir = tempfile::tempdir().expect("create a directory outside any repository");
    let outside = Utf8Path::from_path(outside_dir.path()).expect("the directory is UTF-8");
    assert!(
        SnapshotGit::local()
            .stdout("rev-parse", Some(outside), &["rev-parse", "--git-dir"])
            .is_err(),
        "the fixture directory must not be inside a repository"
    );

    let heads = SnapshotGit::local()
        .with_working_directory(outside)
        .list_heads(fixture.bundle())
        .expect("listing heads needs no repository");

    assert!(
        heads.iter().any(|head| head.object == fixture.commit()),
        "{heads:?}"
    );
}

#[test]
fn verifying_a_header_removes_its_scratch_repository() {
    let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let scratch_parent = Utf8Path::from_path(holder.path()).expect("the scratch parent is UTF-8");

    SnapshotGit::local()
        .with_scratch_parent(scratch_parent)
        .verify_header(fixture.bundle())
        .expect("verify the bundle's header");

    assert_eq!(
        directory_entries(scratch_parent),
        Vec::<String>::new(),
        "the scratch repository must not outlive the check"
    );
}

#[test]
fn a_failed_verification_still_removes_its_scratch_repository() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let scratch_parent = Utf8Path::from_path(holder.path()).expect("the scratch parent is UTF-8");
    let missing = scratch_parent.join("missing.bundle");

    let error = SnapshotGit::local()
        .with_scratch_parent(scratch_parent)
        .verify_header(&missing)
        .expect_err("an absent bundle cannot be verified");

    assert!(
        matches!(&error, ClippySnapshotError::Invalid { .. }),
        "{error}"
    );
    assert_eq!(
        directory_entries(scratch_parent),
        Vec::<String>::new(),
        "the scratch repository must be removed after a failure too"
    );
}

/// A removal that fails after a successful command is reported.
#[test]
fn a_cleanup_failure_after_a_successful_operation_is_reported() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let (_moved, scratch) = unremovable_scratch(holder.path());
    let location = scratch.path().display().to_string();

    let error = super::scratch::finish_scratch(scratch, Ok(42))
        .expect_err("a scratch repository that cannot be removed is a failure");

    match &error {
        ClippySnapshotError::Io { path, reason } => {
            // The error names the scratch directory the adapter could not
            // remove, not the bundle the caller owns.
            assert_eq!(path, &location, "{path}");
            assert!(
                reason.contains("could not remove the scratch repository"),
                "{reason}"
            );
        }
        other => panic!("expected the removal to be reported: {other}"),
    }
}

/// Both failures survive: the command's diagnosis first, the cleanup after it.
#[test]
fn a_cleanup_failure_does_not_hide_a_failed_operation() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let (_moved, scratch) = unremovable_scratch(holder.path());
    let failure = ClippySnapshotError::Git {
        operation: "bundle list-heads",
        message: "fatal: not a git repository".to_owned(),
    };

    let outcome: Result<(), ClippySnapshotError> = Err(failure);
    let error = super::scratch::finish_scratch(scratch, outcome)
        .expect_err("a failed command stays failed");

    match &error {
        ClippySnapshotError::Git { message, .. } => {
            assert!(message.contains("not a git repository"), "{message}");
            assert!(message.contains("could not be removed"), "{message}");
        }
        other => panic!("expected the Git diagnosis to stay primary: {other}"),
    }
}

/// An initialization failure still attempts removal, and reports both.
#[test]
fn a_failed_initialization_still_attempts_cleanup() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    // The scratch path is a regular file by now, so `git init` cannot create a
    // repository there and the later removal cannot empty it either.
    let (_moved, scratch) = unremovable_scratch(holder.path());

    let error = SnapshotGit::local()
        .initialize_scratch(scratch)
        .expect_err("a regular file cannot become a repository");

    let message = error.to_string();
    assert!(message.contains("could not be removed"), "{message}");
}

/// Create a scratch directory whose recorded `TempDir` can no longer be removed.
///
/// The directory is moved aside and a regular file takes its place, because
/// `remove_dir_all` fails on a file with "not a directory" on every platform.
/// A read-only directory would be simpler, but root and some filesystems ignore
/// permission bits, so such a fixture silently skips the test instead of
/// proving the cleanup path.
///
/// The moved directory is returned so it stays alive; the holder removes it
/// with the rest of the fixture when the test ends.
fn unremovable_scratch(parent: &std::path::Path) -> (tempfile::TempDir, tempfile::TempDir) {
    let scratch = tempfile::Builder::new()
        .prefix("unremovable-scratch-")
        .tempdir_in(parent)
        .expect("create the scratch directory");
    std::fs::write(scratch.path().join("held"), "an entry blocks the removal")
        .expect("write the blocking entry");
    let moved = tempfile::Builder::new()
        .prefix("moved-scratch-")
        .tempdir_in(parent)
        .expect("create the directory that holds the moved scratch");
    std::fs::rename(scratch.path(), moved.path().join("scratch"))
        .expect("move the scratch directory out of the way");
    std::fs::write(scratch.path(), "a file is not a directory")
        .expect("leave a file where the scratch directory was");

    (moved, scratch)
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
