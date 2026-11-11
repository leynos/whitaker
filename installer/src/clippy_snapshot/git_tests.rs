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
///
/// Unix only: removal needs write permission on the directory being emptied,
/// so a read-only directory holding an entry cannot be removed. Windows
/// read-only flags do not stop deletion in the same way.
#[cfg(unix)]
#[test]
fn a_cleanup_failure_after_a_successful_operation_is_reported() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let _release = Release(holder.path());
    let scratch = unremovable_scratch(holder.path());
    if !permissions_are_enforced(scratch.path()) {
        return;
    }
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
#[cfg(unix)]
#[test]
fn a_cleanup_failure_does_not_hide_a_failed_operation() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let _release = Release(holder.path());
    let scratch = unremovable_scratch(holder.path());
    if !permissions_are_enforced(scratch.path()) {
        return;
    }
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
#[cfg(unix)]
#[test]
fn a_failed_initialization_still_attempts_cleanup() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let _release = Release(holder.path());
    // The directory is read-only and already holds an entry, so `git init`
    // cannot write its files and a later removal cannot empty it.
    let scratch = unremovable_scratch(holder.path());
    if !permissions_are_enforced(scratch.path()) {
        return;
    }

    let error = SnapshotGit::local()
        .initialize_scratch(scratch)
        .expect_err("a read-only directory cannot become a repository");

    let message = error.to_string();
    assert!(message.contains("could not be removed"), "{message}");
}

/// Create a scratch directory that cannot be removed.
#[cfg(unix)]
fn unremovable_scratch(parent: &std::path::Path) -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let scratch = tempfile::Builder::new()
        .prefix("unremovable-scratch-")
        .tempdir_in(parent)
        .expect("create the scratch directory");
    std::fs::write(scratch.path().join("held"), "an entry blocks the removal")
        .expect("write the blocking entry");
    std::fs::set_permissions(scratch.path(), std::fs::Permissions::from_mode(0o555))
        .expect("make the scratch directory read-only");
    scratch
}

/// Restore write permission under the holder when the test ends.
///
/// Declaring the guard after the holder means it drops first, so a failed
/// assertion cannot leave an unremovable directory behind.
#[cfg(unix)]
struct Release<'a>(&'a std::path::Path);

#[cfg(unix)]
impl Drop for Release<'_> {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;

        let Ok(entries) = std::fs::read_dir(self.0) else {
            return;
        };
        for entry in entries.flatten() {
            // Best effort: a restore failure must not mask the test result.
            drop(std::fs::set_permissions(
                entry.path(),
                std::fs::Permissions::from_mode(0o755),
            ));
        }
    }
}

/// Whether the read-only permissions actually block a write.
///
/// Root and some filesystems ignore permission bits, and then the read-only
/// fixture proves nothing. The probe writes into the directory once; success
/// means the bits are not enforced and the caller must skip its assertions.
#[cfg(unix)]
fn permissions_are_enforced(scratch: &std::path::Path) -> bool {
    let probe = scratch.join("probe");
    let blocked = std::fs::write(&probe, "").is_err();
    if !blocked {
        // Remove the probe so the caller's fixture stays as it was.
        drop(std::fs::remove_file(&probe));
    }
    blocked
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
