//! The prepared value's promise: it names only a path Dylint can load.
//!
//! Every test builds a real path list and asks [`DirectLibraryPath::new`] to
//! accept or refuse it, so the tests exercise the same inspection Dylint's
//! search will later rely on rather than a restatement of its rules.

use std::ffi::OsString;

use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;
use whitaker_installer::{crate_name::CrateName, stager::Stager};

use super::{DirectLibraryPath, NOT_A_DIRECTORY, NOT_A_LIBRARY, NOT_ABSOLUTE};
use crate::ports::CheckPreflightError;

/// The staged name of the Whitaker suite library for `toolchain`.
fn staged_name(toolchain: &str) -> String {
    Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(super::SUITE_CRATE))
}

/// A directory holding a suite library for each named toolchain.
fn holding(toolchains: &[&str]) -> (TempDir, OsString) {
    let directory = TempDir::new().expect("a temporary directory is available");
    for toolchain in toolchains {
        std::fs::write(directory.path().join(staged_name(toolchain)), b"")
            .expect("the suite file can be created");
    }
    let path = directory.path().as_os_str().to_owned();
    (directory, path)
}

/// Join `directories` into one `DYLINT_LIBRARY_PATH` value.
fn path_list(directories: &[&std::path::Path]) -> OsString {
    std::env::join_paths(directories.iter().map(|path| path.as_os_str()))
        .expect("temporary paths are representable")
}

#[test]
fn a_path_naming_the_suite_becomes_a_prepared_value() {
    let (_directory, path) = holding(&["nightly-2026-05-28"]);

    let prepared =
        DirectLibraryPath::new(path.clone()).expect("one staged suite makes the path loadable");

    assert_eq!(
        prepared.as_os_str(),
        path,
        "the prepared value must keep the path it validated"
    );
}

/// Count a directory listed twice as the one library it holds.
#[test]
fn a_repeated_directory_counts_once() {
    let (directory, _path) = holding(&["nightly-2026-05-28"]);
    let repeated = path_list(&[directory.path(), directory.path()]);

    let prepared = DirectLibraryPath::new(repeated);

    assert!(
        prepared.is_ok(),
        "Dylint deduplicates discovered libraries, so a repeated directory is not ambiguous, got \
         {prepared:?}"
    );
}

/// Keep two suites ambiguous even when a directory is listed twice.
#[test]
fn a_repeated_directory_does_not_hide_two_suites() {
    let (directory, _path) = holding(&["nightly-2026-05-28", "nightly-2026-05-29"]);
    let repeated = path_list(&[directory.path(), directory.path()]);

    let refused = DirectLibraryPath::new(repeated);

    assert_eq!(
        refused,
        Err(CheckPreflightError::UserPathAmbiguous),
        "two distinct suites are ambiguous however many times their directory is listed"
    );
}

/// Refuse a relative entry, which Dylint refuses as not absolute.
#[test]
fn a_relative_entry_is_refused() {
    let refused = DirectLibraryPath::new(OsString::from("lints"));

    assert_eq!(
        refused,
        Err(CheckPreflightError::UserPathUnusable {
            path: "lints".to_owned(),
            reason: NOT_ABSOLUTE,
        }),
        "Dylint rejects a relative entry before searching anything"
    );
}

/// Refuse a missing directory, which Dylint refuses as not a directory.
#[test]
fn a_missing_directory_is_refused() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let absent = directory.path().join("removed-by-a-later-uninstall");
    let path = path_list(&[&absent]);
    let expected = CheckPreflightError::UserPathUnusable {
        path: absent.display().to_string(),
        reason: NOT_A_DIRECTORY,
    };

    let refused = DirectLibraryPath::new(path);

    assert_eq!(
        refused,
        Err(expected),
        "Dylint rejects a list entry that is not a directory, so check must refuse first"
    );
}

/// Refuse a suite-named directory, which Dylint cannot load.
#[test]
fn a_suite_named_directory_is_refused() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let suite = directory.path().join(staged_name("nightly-2026-05-28"));
    std::fs::create_dir(&suite).expect("the suite-named directory can be created");
    let expected = CheckPreflightError::UserPathUnusable {
        path: suite.display().to_string(),
        reason: NOT_A_LIBRARY,
    };

    let refused = DirectLibraryPath::new(path_list(&[directory.path()]));

    assert_eq!(
        refused,
        Err(expected),
        "a directory is not something Dylint can load as the suite"
    );
}

/// Call a suite file and a suite-named directory ambiguous, as Dylint does.
#[test]
fn a_suite_named_directory_beside_a_suite_file_is_ambiguous() {
    let (directory, _path) = holding(&["nightly-2026-05-28"]);
    std::fs::create_dir(directory.path().join(staged_name("nightly-2026-05-29")))
        .expect("the suite-named directory can be created");

    let refused = DirectLibraryPath::new(path_list(&[directory.path()]));

    assert_eq!(
        refused,
        Err(CheckPreflightError::UserPathAmbiguous),
        "Dylint discovers candidates by file name, so both entries reach its selection"
    );
}

/// Accept a symlink to a staged suite file.
#[cfg(unix)]
#[test]
fn a_symlink_to_a_suite_file_is_accepted() {
    let (directory, _path) = holding(&["nightly-2026-05-28"]);
    let target = directory.path().join(staged_name("nightly-2026-05-28"));
    let linked = TempDir::new().expect("a temporary directory is available");
    std::os::unix::fs::symlink(
        &target,
        linked.path().join(staged_name("nightly-2026-05-28")),
    )
    .expect("the symlink can be created");

    let prepared = DirectLibraryPath::new(path_list(&[linked.path()]));

    assert!(
        prepared.is_ok(),
        "a symlink to a regular suite file is a library Dylint can load, got {prepared:?}"
    );
}

#[rstest]
#[case::no_suite(&[])]
#[case::two_suites(&["nightly-2026-05-28", "nightly-2026-05-29"])]
fn a_path_dylint_could_not_load_is_refused(#[case] toolchains: &[&str]) {
    let (_directory, path) = holding(toolchains);

    let refused = DirectLibraryPath::new(path);

    assert!(
        matches!(
            refused,
            Err(CheckPreflightError::UserPathHasNoSuite | CheckPreflightError::UserPathAmbiguous)
        ),
        "a path Dylint could not load must not become a prepared value: {refused:?}"
    );
}
