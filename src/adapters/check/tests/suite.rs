//! Which library paths the adapter will let Dylint load the suite from.

use std::ffi::{OsStr, OsString};

use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;
use whitaker_installer::{crate_name::CrateName, resolution::SUITE_CRATE, stager::Stager};

use super::super::{
    LibraryMode,
    mode_for,
    suite::{has_suite_name, suite_present},
};
use crate::ports::CheckPreflightError;

/// The staged file name a Whitaker suite library has on this platform.
fn suite_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-28")
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// The staged file name the same suite has for another toolchain.
fn other_suite_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-29")
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// Join `directories` into one platform path list.
fn path_list(directories: &[&TempDir]) -> OsString {
    std::env::join_paths(directories.iter().map(|dir| dir.path()))
        .expect("temporary paths are representable")
}

#[test]
fn a_directory_holding_the_suite_satisfies_the_path() {
    let directory = TempDir::new().expect("a temporary directory is available");
    std::fs::write(directory.path().join(suite_filename()), b"")
        .expect("the suite file can be created");

    let result = suite_present(&path_list(&[&directory]));

    assert!(
        result.is_ok(),
        "a staged suite library must satisfy the path, got {result:?}"
    );
}

#[rstest]
#[case::empty_directory(&[])]
#[case::another_library(&["libmodule_max_lines@nightly-2026-05-28.so"])]
#[case::unrelated_file(&["notes.txt"])]
fn a_directory_without_the_suite_does_not_satisfy_the_path(#[case] names: &[&str]) {
    let directory = TempDir::new().expect("a temporary directory is available");
    for name in names {
        std::fs::write(directory.path().join(name), b"").expect("the file can be created");
    }

    let result = suite_present(&path_list(&[&directory]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathHasNoSuite),
        "only a Whitaker suite library may satisfy the path"
    );
}

#[test]
fn every_directory_of_the_path_list_is_searched() {
    let empty = TempDir::new().expect("a temporary directory is available");
    let holding = TempDir::new().expect("a temporary directory is available");
    std::fs::write(holding.path().join(suite_filename()), b"").expect("the file can be created");

    let result = suite_present(&path_list(&[&empty, &holding]));

    assert!(
        result.is_ok(),
        "a later directory must still be searched, got {result:?}"
    );
}

#[test]
fn a_missing_directory_is_skipped_rather_than_failing() {
    let missing = TempDir::new().expect("a temporary directory is available");
    let absent = missing.path().join("removed-by-a-later-uninstall");
    let path = OsString::from(absent.as_os_str());

    let result = suite_present(&path);

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathHasNoSuite),
        "a directory an uninstall removed is not an error"
    );
}

#[test]
fn an_unreadable_directory_is_a_host_fault() {
    let directory = TempDir::new().expect("a temporary directory is available");
    // A regular file where a directory is expected makes the read fail with
    // something other than "not found", which is the case under test.
    let not_a_directory = directory.path().join("i-am-a-file");
    std::fs::write(&not_a_directory, b"").expect("the file can be created");
    let path = OsString::from(not_a_directory.as_os_str());

    let result = suite_present(&path);

    assert!(
        matches!(
            result,
            Err(CheckPreflightError::Bootstrap(
                whitaker_installer::bootstrap::BootstrapError::Io { .. }
            ))
        ),
        "a directory that cannot be read is a host fault, got {result:?}"
    );
}

#[rstest]
#[case::unset(None, LibraryMode::Recorded)]
#[case::empty(Some(OsString::new()), LibraryMode::Recorded)]
#[case::set(
    Some(OsString::from("/one:/two")),
    LibraryMode::UserManaged(OsString::from("/one:/two"))
)]
fn the_library_mode_follows_a_non_empty_setting(
    #[case] explicit: Option<OsString>,
    #[case] expected: LibraryMode,
) {
    assert_eq!(
        mode_for(explicit),
        expected,
        "an unset or empty setting must leave the recorded installation authoritative"
    );
}

#[test]
fn an_unreadable_file_name_is_not_a_suite_library() {
    assert!(
        !has_suite_name(OsStr::new("libwhitaker_suite.so")),
        "a library without a toolchain suffix is not a staged suite"
    );
}

#[test]
fn two_suite_libraries_on_the_path_are_ambiguous() {
    let directory = TempDir::new().expect("a temporary directory is available");
    for name in [suite_filename(), other_suite_filename()] {
        std::fs::write(directory.path().join(name), b"").expect("the suite file can be created");
    }

    let result = suite_present(&path_list(&[&directory]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathAmbiguous),
        "Dylint refuses to choose between two suites, so check must refuse first"
    );
}

#[test]
fn a_directory_named_like_the_suite_is_not_the_suite() {
    let directory = TempDir::new().expect("a temporary directory is available");
    std::fs::create_dir(directory.path().join(suite_filename()))
        .expect("the suite-named directory can be created");

    let result = suite_present(&path_list(&[&directory]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathHasNoSuite),
        "a directory is not a library Dylint could load"
    );
}
