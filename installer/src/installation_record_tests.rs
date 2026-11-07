//! Tests for exact installed-path persistence.

use std::path::PathBuf;

use camino::Utf8Path;
use tempfile::TempDir;

use super::{
    InstallationRecordError,
    RecordContents,
    installed_paths,
    read_installation_record,
    record_installation,
};
use crate::dirs::{BaseDirs, MockBaseDirs};

fn mock_dirs(data: PathBuf) -> MockBaseDirs {
    let mut dirs = MockBaseDirs::new();
    dirs.expect_whitaker_data().return_const(Some(data));
    dirs
}

#[test]
fn installation_paths_preserve_toolchains_and_replace_the_same_channel() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let stable = temporary.path().join("stable");
    let nightly = temporary.path().join("nightly");
    let replacement = temporary.path().join("stable-replacement");
    std::fs::create_dir_all(&stable).expect("stable staging directory should be created");
    std::fs::create_dir_all(&nightly).expect("nightly staging directory should be created");
    std::fs::create_dir_all(&replacement).expect("replacement staging directory should be created");
    let expected_nightly = Utf8Path::from_path(&nightly)
        .expect("nightly path should be UTF-8")
        .canonicalize_utf8()
        .expect("nightly path should be canonicalized");
    let expected_replacement = Utf8Path::from_path(&replacement)
        .expect("replacement path should be UTF-8")
        .canonicalize_utf8()
        .expect("replacement path should be canonicalized");
    let dirs = mock_dirs(temporary.path().join("data"));

    record_installation(
        &dirs,
        "stable",
        Utf8Path::from_path(&stable).expect("stable path should be UTF-8"),
    )
    .expect("stable path should be recorded");
    record_installation(
        &dirs,
        "nightly",
        Utf8Path::from_path(&nightly).expect("nightly path should be UTF-8"),
    )
    .expect("nightly path should be recorded");
    record_installation(
        &dirs,
        "stable",
        Utf8Path::from_path(&replacement).expect("replacement path should be UTF-8"),
    )
    .expect("replacement path should be recorded");

    let recorded = installed_paths(&dirs).expect("recorded paths should load");
    assert_eq!(recorded.len(), 2);
    assert!(recorded.contains(&expected_replacement));
    assert!(recorded.contains(&expected_nightly));
}

#[test]
fn installed_paths_ignore_stale_records_and_missing_files() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let staging = temporary.path().join("staging");
    std::fs::create_dir_all(&staging).expect("staging directory should be created");
    let dirs = mock_dirs(temporary.path().join("data"));
    assert!(
        installed_paths(&dirs)
            .expect("a missing record should be an empty inventory")
            .is_empty()
    );
    record_installation(
        &dirs,
        "gone",
        Utf8Path::from_path(&staging).expect("stale path should be UTF-8"),
    )
    .expect("existing staging directory should be recorded");
    std::fs::remove_dir(&staging).expect("staging directory should be removed");
    assert!(
        installed_paths(&dirs)
            .expect("stale records should be ignored")
            .is_empty()
    );
}

#[test]
fn a_read_record_distinguishes_missing_empty_stale_and_current() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let data = temporary.path().join("data");
    let staging = temporary.path().join("staging");
    std::fs::create_dir_all(&staging).expect("staging directory should be created");
    let dirs = mock_dirs(data.clone());

    let missing = read_installation_record(&dirs).expect("a missing record should load");
    assert_eq!(missing.contents(), RecordContents::Missing);

    std::fs::create_dir_all(&data).expect("data directory should be created");
    std::fs::write(data.join(super::RECORD_FILENAME), "{}")
        .expect("an empty record should be written");
    let empty = read_installation_record(&dirs).expect("an empty record should load");
    assert_eq!(empty.contents(), RecordContents::Empty);

    record_installation(
        &dirs,
        "nightly",
        Utf8Path::from_path(&staging).expect("staging path should be UTF-8"),
    )
    .expect("staging path should be recorded");
    let current = read_installation_record(&dirs).expect("a current record should load");
    assert_eq!(current.contents(), RecordContents::Current);

    std::fs::remove_dir(&staging).expect("staging directory should be removed");
    let stale = read_installation_record(&dirs).expect("a stale record should load");
    assert_eq!(stale.contents(), RecordContents::Stale);
    assert!(
        stale.into_surviving_paths().is_empty(),
        "a stale record should contribute no paths"
    );
}

#[test]
fn successful_install_replaces_a_malformed_record() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let data = temporary.path().join("data");
    let staging = temporary.path().join("staging");
    std::fs::create_dir_all(&data).expect("data directory should be created");
    std::fs::create_dir_all(&staging).expect("staging directory should be created");
    std::fs::write(data.join(super::RECORD_FILENAME), "not JSON")
        .expect("malformed record should be written");
    let dirs = mock_dirs(data);

    assert!(matches!(
        installed_paths(&dirs),
        Err(InstallationRecordError::Parse { .. })
    ));
    record_installation(
        &dirs,
        "nightly",
        Utf8Path::from_path(&staging).expect("staging path should be UTF-8"),
    )
    .expect("successful install should replace malformed record");

    assert_eq!(
        installed_paths(&dirs)
            .expect("new record should load")
            .len(),
        1
    );
}

#[test]
fn the_record_api_uses_only_the_whitaker_data_directory() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let staging = temporary.path().join("staging");
    std::fs::create_dir_all(&staging).expect("staging directory should be created");
    let dirs = mock_dirs(temporary.path().join("data"));

    record_installation(
        &dirs,
        "nightly",
        Utf8Path::from_path(&staging).expect("staging path should be UTF-8"),
    )
    .expect("staging path should be recorded");

    let data_dir = dirs.whitaker_data().expect("data directory should exist");
    assert!(data_dir.join("installed_lint_paths.json").is_file());
}
