//! Which library paths the adapter will let Dylint load the suite from.

use std::ffi::{OsStr, OsString};

use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;
use whitaker_installer::{crate_name::CrateName, resolution::SUITE_CRATE, stager::Stager};

use super::super::{
    LibraryMode,
    mode_for,
    prepared_suite_available,
    suite::{suite_available, suite_present},
};
use crate::{library_path::has_suite_name, ports::CheckPreflightError};

/// Build the staged file name a Whitaker suite library has on this platform.
fn suite_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-28")
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// Build the staged file name the same suite has for another toolchain.
fn other_suite_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-29")
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// The staged file name of a lint crate that is not the suite.
///
/// Generated rather than written as a literal so the case reaches the crate
/// comparison on every host: a hard-coded `.so` suffix is not a staged name on
/// macOS or Windows, and the parser would reject the file before comparing.
fn other_library_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-28")
        .staged_filename(&CrateName::from("module_max_lines"))
}

/// Join `directories` into one platform path list.
fn path_list(directories: &[&TempDir]) -> OsString {
    std::env::join_paths(directories.iter().map(|dir| dir.path()))
        .expect("temporary paths are representable")
}

/// Accept a path whose directory holds the staged suite.
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

/// Refuse a directory that holds anything but the suite.
#[rstest]
#[case::empty_directory(&[])]
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

/// Refuse another lint crate's staged library as if it were the suite.
#[test]
fn another_lint_crates_library_does_not_satisfy_the_path() {
    let directory = TempDir::new().expect("a temporary directory is available");
    std::fs::write(directory.path().join(other_library_filename()), b"")
        .expect("the other library file can be created");

    let result = suite_present(&path_list(&[&directory]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathHasNoSuite),
        "a staged library for another crate must not stand in for the suite"
    );
}

/// Keep searching after an empty directory and find the suite later on.
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

/// Keep searching after a directory that no longer exists.
///
/// The holding directory is second on the list on purpose: stopping at the
/// first entry would pass a test whose only entry is missing, but not this one.
#[test]
fn a_missing_directory_is_skipped_rather_than_failing() {
    let holding = TempDir::new().expect("a temporary directory is available");
    std::fs::write(holding.path().join(suite_filename()), b"").expect("the file can be created");
    let missing = TempDir::new().expect("a temporary directory is available");
    let absent = missing.path().join("removed-by-a-later-uninstall");
    let path = std::env::join_paths([absent.as_os_str(), holding.path().as_os_str()])
        .expect("temporary paths are representable");

    let result = suite_present(&path);

    assert!(
        result.is_ok(),
        "a directory an uninstall removed must not stop the search, got {result:?}"
    );
}

/// Report an unreadable directory as a host fault rather than a missing suite.
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

/// Treat only a non-empty setting as the user's own library path.
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

/// Reject a suite library name that carries no toolchain suffix.
#[test]
fn a_library_without_a_toolchain_suffix_is_not_the_suite() {
    assert!(
        !has_suite_name(OsStr::new("libwhitaker_suite.so")),
        "a readable name missing the toolchain suffix is not a staged suite"
    );
}

/// Refuse a path that holds suites for two toolchains.
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

/// Keep a recorded installation with a suite per toolchain runnable.
///
/// Readiness asks only whether the suite is there, because a recorded
/// installation keeps a library for each toolchain it was built for. The run
/// re-check must ask the same question: the stricter "exactly one" rule, shown
/// here to refuse the same directory, would report an approved installation as
/// a provisioning defect the repair cannot mend.
#[test]
fn a_two_toolchain_installation_stays_available_to_the_run() {
    let directory = TempDir::new().expect("a temporary directory is available");
    for name in [suite_filename(), other_suite_filename()] {
        std::fs::write(directory.path().join(name), b"").expect("the suite file can be created");
    }
    let path = directory.path().as_os_str();

    assert_eq!(
        suite_available(path),
        Ok(true),
        "readiness must approve an installation holding a suite per toolchain"
    );
    assert_eq!(
        suite_present(path),
        Err(CheckPreflightError::UserPathAmbiguous),
        "the exactly-one rule is what the run re-check must not use here"
    );
    let rerun = prepared_suite_available(path);
    assert!(
        rerun.is_ok(),
        "the run must accept the installation readiness approved, got {rerun:?}"
    );
}

/// Refuse a directory that merely carries the suite's file name.
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
