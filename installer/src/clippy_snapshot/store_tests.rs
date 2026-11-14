//! The filesystem adapter keeps its own promises, not just the orchestration's.
//!
//! These tests hold the contracts `store.rs` states directly: which diagnosis
//! a record it cannot accept produces, that a written record reads back
//! unchanged, that the root lock is exclusive, and that a staged generation
//! exists only until it is published or abandoned. The orchestration that uses
//! these operations is covered separately in `orchestration_tests/`, and the
//! acceptance rules in `policy_tests.rs`.

use std::{collections::BTreeSet, sync::mpsc, time::Duration};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{
    error::ClippySnapshotError,
    generation::{GenerationMetadata, METADATA_FILE, METADATA_SCHEMA},
    ports::{MetadataStore, SnapshotStore},
    store::ProductionStore,
};

/// One record the adapter must reject, and the diagnosis it must give.
struct Rejected {
    /// What the metadata file holds, or `None` when there is no file at all.
    record: Option<&'static str>,
    /// The sentence the adapter composes itself.
    diagnosis: String,
    /// Whether `diagnosis` is only the prefix of the reported text.
    ///
    /// A missing file's sentence ends in the operating system's own wording,
    /// which differs between platforms, so those rows compare the prefix and
    /// require the operating system's reason to be kept. Every sentence the
    /// adapter writes itself is compared whole.
    ends_in_os_wording: bool,
}

#[rstest]
#[case::no_record(Rejected {
    record: None,
    diagnosis: format!("{METADATA_FILE} is unreadable: "),
    ends_in_os_wording: true,
})]
#[case::malformed_json(Rejected {
    record: Some(r#"{"schema_version":"#),
    diagnosis: format!("{METADATA_FILE} is malformed: EOF while parsing a value at line 1 column 18"),
    ends_in_os_wording: false,
})]
#[case::unknown_schema(Rejected {
    record: Some(
        r#"{"schema_version":99,"commit":"a","toolchain":"b",
            "selected_revision":"c","dylint_version":"d","bundle_sha256":"e"}"#,
    ),
    diagnosis: format!("{METADATA_FILE} has schema 99, expected {METADATA_SCHEMA}"),
    ends_in_os_wording: false,
})]
fn a_rejected_record_names_what_is_wrong(#[case] rejected: Rejected) {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let generation = generation_directory(&holder, rejected.record);

    let error = ProductionStore
        .read(&generation)
        .expect_err("the record is rejected");

    let ClippySnapshotError::Invalid { path, check } = &error else {
        panic!("expected an invalid record: {error}");
    };
    assert_eq!(path, generation.as_str(), "{error}");
    if rejected.ends_in_os_wording {
        let detail = check
            .strip_prefix(&rejected.diagnosis)
            .unwrap_or_else(|| panic!("{check} must start with the adapter's own sentence"));
        assert!(
            !detail.is_empty(),
            "the operating system's reason must be kept: {check}"
        );
    } else {
        assert_eq!(check, &rejected.diagnosis);
    }
}

#[test]
fn a_written_record_reads_back_unchanged() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let generation = generation_directory(&holder, None);
    let metadata = GenerationMetadata {
        schema_version: METADATA_SCHEMA,
        commit: "e4608a25c2e20116fbc1e97a51a73f094a1db7a1".to_owned(),
        toolchain: "nightly-2026-05-28".to_owned(),
        selected_revision: "e4608a25c2e20116fbc1e97a51a73f094a1db7a1".to_owned(),
        dylint_version: "6.3.0".to_owned(),
        bundle_sha256: "a".repeat(64),
    };

    ProductionStore
        .write(&generation, &metadata)
        .expect("write the record");
    let read = ProductionStore
        .read(&generation)
        .expect("read the record back");

    assert_eq!(read, metadata);
    assert_eq!(
        entries(&generation),
        BTreeSet::from([METADATA_FILE.to_owned()]),
        "the temporary file used for the write must not survive it"
    );
}

#[test]
fn the_root_lock_excludes_a_second_holder() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let root = utf8(holder.path()).to_owned();

    let held = ProductionStore.lock(&root).expect("take the root lock");
    let (acquired, waiting) = mpsc::channel();

    std::thread::scope(|scope| {
        let contender = scope.spawn(|| {
            let outcome = ProductionStore.lock(&root).map(drop);
            // The receiver only waits for the signal, so a closed channel is
            // not a failure worth reporting.
            let _signalled = acquired.send(());
            outcome
        });

        assert!(
            waiting.recv_timeout(Duration::from_millis(250)).is_err(),
            "a held root lock must keep the second acquisition waiting"
        );
        drop(held);
        waiting
            .recv_timeout(Duration::from_secs(30))
            .expect("the released lock is acquirable");
        contender
            .join()
            .expect("the contender does not panic")
            .expect("the second acquisition succeeds");
    });
}

#[test]
fn publishing_moves_the_staged_generation_into_place() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let root = utf8(holder.path()).to_owned();
    let staged = ProductionStore
        .staging(&root, ".incoming-")
        .expect("stage a generation");
    let staging = staged.path().to_owned();
    std::fs::write(staging.join("main"), "objects").expect("populate the staged generation");
    let generation = root.join("generation");

    ProductionStore
        .publish(&staging, &generation)
        .expect("publish the staged generation");

    assert!(
        !staging.exists(),
        "the staging directory must not survive publication"
    );
    assert_eq!(
        std::fs::read_to_string(generation.join("main")).expect("read the published file"),
        "objects"
    );
    drop(staged);
    assert!(
        generation.is_dir(),
        "dropping the handle after publication must not disturb the generation"
    );
}

#[test]
fn an_unpublished_staging_directory_is_removed_on_drop() {
    let holder = tempfile::tempdir().expect("create a temporary directory");
    let root = utf8(holder.path()).to_owned();
    let staged = ProductionStore
        .staging(&root, ".incoming-")
        .expect("stage a generation");
    let staging = staged.path().to_owned();
    std::fs::write(staging.join("main"), "objects").expect("populate the staged generation");
    assert!(
        staging.is_dir(),
        "the staging directory exists while it is held"
    );

    drop(staged);

    assert!(
        !staging.exists(),
        "an abandoned staging directory must not outlive the install"
    );
}

/// Create `holder/generation`, holding `record` when one is given.
fn generation_directory(holder: &tempfile::TempDir, record: Option<&str>) -> Utf8PathBuf {
    let generation = utf8(holder.path()).join("generation");
    std::fs::create_dir(&generation).expect("create the generation directory");
    if let Some(contents) = record {
        std::fs::write(generation.join(METADATA_FILE), contents).expect("write the record");
    }
    generation
}

/// Return the entry names of `directory`, so a stray file is visible.
fn entries(directory: &Utf8Path) -> BTreeSet<String> {
    std::fs::read_dir(directory)
        .expect("read the generation directory")
        .map(|entry| {
            entry
                .expect("read a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// Require a UTF-8 path, which the adapter's signatures take.
fn utf8(path: &std::path::Path) -> &Utf8Path {
    Utf8Path::from_path(path).expect("the temporary directory is UTF-8")
}
