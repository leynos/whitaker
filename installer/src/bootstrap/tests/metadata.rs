//! How the installed bootstrap record is written, read and refused.
//!
//! The record is the only description an installation leaves behind, so its
//! tests cover both directions: a record Whitaker wrote must read back
//! unchanged and keep naming the same suite, and a record it cannot trust must
//! be refused with the advice that matches the defect.

use camino::Utf8Path;

use super::temp_root;
use crate::bootstrap::{BOOTSTRAP_SCHEMA, BootstrapError, BootstrapMetadata, StoredPath};

/// A record naming `library_dir` at the schema this release writes.
fn record(library_dir: StoredPath) -> BootstrapMetadata {
    BootstrapMetadata {
        schema_version: BOOTSTRAP_SCHEMA,
        dylint_version: "6.3.0".to_owned(),
        snapshot_commit: "e4608a25c2e20116fbc1e97a51a73f094a1db7a1".to_owned(),
        toolchain: "nightly-2026-05-28".to_owned(),
        library_dir,
        suite_version: None,
    }
}

#[test]
fn metadata_round_trips_and_relocates() {
    let (_dir, root) = temp_root();
    let path = root.join("whitaker-bootstrap/bootstrap.json");
    let written = record(StoredPath::DataRootRelative("whitaker/lints".into()));

    written.write(&path).expect("write metadata");
    let read = BootstrapMetadata::read(&path).expect("read metadata");

    assert_eq!(read, written);
    assert_eq!(
        read.library_dir.resolve(Utf8Path::new("/moved")),
        Utf8Path::new("/moved/whitaker/lints")
    );
}

#[test]
fn a_recorded_suite_pin_survives_a_round_trip() {
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    let mut written = record(StoredPath::DataRootRelative("whitaker/lints".into()));
    written.suite_version = Some("v0.2.7".to_owned());

    written.write(&path).expect("write metadata");
    let read = BootstrapMetadata::read(&path).expect("read metadata");

    assert_eq!(
        read.suite_version.as_deref(),
        Some("v0.2.7"),
        "a repair must be able to rebuild from the reference the install used"
    );
}

#[test]
fn an_unpinned_record_omits_the_suite_pin() {
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    record(StoredPath::DataRootRelative("whitaker/lints".into()))
        .write(&path)
        .expect("write metadata");

    let json = std::fs::read_to_string(&path).expect("read the written record");

    assert!(
        !json.contains("suite_version"),
        "an unpinned install must not claim a pin: {json}"
    );
}

#[test]
fn a_record_written_before_pinning_still_parses() {
    // The field was added after the first installations shipped, so an existing
    // record has no `suite_version` key at all and must still be usable.
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    std::fs::write(
        &path,
        concat!(
            "{\n",
            "  \"schema_version\": 1,\n",
            "  \"dylint_version\": \"6.3.0\",\n",
            "  \"snapshot_commit\": \"e4608a25\",\n",
            "  \"toolchain\": \"nightly-2026-05-28\",\n",
            "  \"library_dir\": {\"data_root_relative\": \"whitaker/lints\"}\n",
            "}\n",
        ),
    )
    .expect("write an older record");

    let read = BootstrapMetadata::read(&path).expect("an older record is still readable");

    assert_eq!(
        read.suite_version, None,
        "a record without the field describes an unpinned install"
    );
}

#[test]
fn missing_metadata_points_at_the_installer() {
    let (_dir, root) = temp_root();
    let error = BootstrapMetadata::read(&root.join("bootstrap.json")).expect_err("absent");

    assert!(
        matches!(error, BootstrapError::NotInstalled { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("run whitaker-installer"));
}

#[test]
fn rejects_metadata_from_another_schema() {
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    let mut future = record(StoredPath::Absolute("/x".into()));
    future.schema_version = BOOTSTRAP_SCHEMA + 1;
    future.write(&path).expect("write metadata");

    let error = BootstrapMetadata::read(&path).expect_err("future schema is rejected");

    assert!(error.to_string().contains("is not supported"), "{error}");
}

/// Report a record whose bytes are not UTF-8 as malformed, not as unreadable.
///
/// The distinction decides the advice: malformed content is reinstallable,
/// whereas an I/O failure is a host fault. Reading the file as text collapsed
/// the two, because the decode failure arrived as a read error.
#[test]
fn a_record_that_is_not_utf8_is_malformed() {
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    std::fs::write(&path, b"{\"schema_version\": 1, \"note\": \"\xff\"}")
        .expect("the damaged record can be written");

    let error = BootstrapMetadata::read(&path).expect_err("damaged content is not metadata");

    assert!(
        matches!(error, BootstrapError::Metadata { .. }),
        "damaged bytes are malformed metadata rather than a host fault: {error:?}"
    );
}
