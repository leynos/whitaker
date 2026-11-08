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

#[test]
fn a_clone_that_cannot_start_is_an_infrastructure_failure() {
    // An empty `PATH` leaves the child unable to find `git` at all, which is
    // the same class of fault as a missing binary: the clone never ran, so the
    // bundle is not implicated.
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

    assert!(heads.contains(fixture.commit()), "{heads}");
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
