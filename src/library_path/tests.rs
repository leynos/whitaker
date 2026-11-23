//! The pure rules that judge what a Dylint library path list holds.
//!
//! Every test states the listing the adapter would have produced and asks
//! [`classify`] or the prepared value's constructor to judge it, so the rules
//! are pinned without a temporary directory and without depending on what the
//! host happens to hold. The seam that produces those listings is tested at
//! its own boundary.

use std::{ffi::OsString, path::PathBuf};

use camino::Utf8PathBuf;
use rstest::rstest;
use whitaker_installer::{crate_name::CrateName, stager::Stager};

use super::{
    DirectLibraryPath,
    EntryListing,
    EntryReport,
    ListedFile,
    NOT_A_DIRECTORY,
    NOT_A_LIBRARY,
    NOT_ABSOLUTE,
    SuitePresence,
    classify,
};
use crate::ports::CheckPreflightError;

/// The staged name of the Whitaker suite library for `toolchain`.
fn staged_name(toolchain: &str) -> String {
    Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(super::SUITE_CRATE))
}

/// A path-list entry reporting `listing` for `path`.
fn report(path: &str, listing: EntryListing) -> EntryReport {
    EntryReport {
        path: PathBuf::from(path),
        listing,
    }
}

/// One directory entry under `directory`, as the listing would report it.
fn listed(directory: &str, name: &str, is_regular_file: bool) -> ListedFile {
    ListedFile {
        path: PathBuf::from(directory).join(name),
        is_regular_file,
    }
}

/// A path-list entry whose directory holds `files`.
fn directory_report(path: &str, files: Vec<ListedFile>) -> EntryReport {
    report(path, EntryListing::Directory(files))
}

/// Refuse a relative entry before asking any filesystem question.
#[test]
fn a_relative_entry_is_unusable_before_anything_is_read() {
    let reports = [report("lints", EntryListing::Uninspected)];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: "lints".to_owned(),
            reason: NOT_ABSOLUTE,
        },
        "Dylint rejects a relative entry before searching anything"
    );
}

/// Refuse an entry that names nothing on the host.
#[test]
fn a_missing_entry_is_not_a_directory() {
    let reports = [report("/lints", EntryListing::Missing)];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: "/lints".to_owned(),
            reason: NOT_A_DIRECTORY,
        },
        "Dylint rejects a list entry that is not a directory"
    );
}

/// Refuse an entry that names something other than a directory.
#[test]
fn a_non_directory_entry_is_not_a_directory() {
    let reports = [report("/lints/file", EntryListing::NotADirectory)];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: "/lints/file".to_owned(),
            reason: NOT_A_DIRECTORY,
        },
        "a file on the list is not a directory Dylint can search"
    );
}

/// Report the first defective entry in list order.
///
/// The old inspection returned as soon as an entry could not be searched, so
/// a missing entry before a relative one reported the missing one. The pure
/// rules must preserve that order rather than reordering the defects.
#[test]
fn the_first_defective_entry_in_list_order_is_reported() {
    let reports = [
        report("/gone", EntryListing::Missing),
        report("lints", EntryListing::Uninspected),
    ];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: "/gone".to_owned(),
            reason: NOT_A_DIRECTORY,
        },
        "the first entry Dylint could not search is the one reported"
    );
}

/// Ignore a directory that holds nothing named for the suite.
#[test]
fn a_directory_without_a_suite_is_absent() {
    let reports = [directory_report(
        "/lints",
        vec![listed("/lints", "notes.txt", true)],
    )];

    assert_eq!(
        classify(&reports),
        SuitePresence::Absent,
        "only an entry named for the suite is a candidate"
    );
}

/// Accept a suite-named candidate the listing calls a regular file.
///
/// The adapter resolves symlinks before reporting, so a linked suite arrives
/// with `is_regular_file: true` exactly as a staged file does; the rules must
/// not distinguish the two.
#[rstest]
#[case::staged_file("/lints")]
#[case::followed_symlink("/linked")]
fn a_suite_named_regular_file_is_exactly_one(#[case] directory: &str) {
    let name = staged_name("nightly-2026-05-28");
    let reports = [directory_report(
        directory,
        vec![listed(directory, &name, true)],
    )];

    assert_eq!(
        classify(&reports),
        SuitePresence::ExactlyOne,
        "a regular file named for the suite is a library Dylint can load"
    );
}

/// Refuse a suite-named candidate the listing says is not a regular file.
#[test]
fn a_suite_named_directory_is_not_a_library() {
    let directory = "/lints";
    let name = staged_name("nightly-2026-05-28");
    let suite = PathBuf::from(directory).join(&name);
    let reports = [directory_report(
        directory,
        vec![listed(directory, &name, false)],
    )];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: suite.display().to_string(),
            reason: NOT_A_LIBRARY,
        },
        "a directory is not something Dylint can load as the suite"
    );
}

/// Refuse two candidates, however they are shaped.
#[test]
fn two_suite_files_are_ambiguous() {
    let directory = "/lints";
    let reports = [directory_report(
        directory,
        vec![
            listed(directory, &staged_name("nightly-2026-05-28"), true),
            listed(directory, &staged_name("nightly-2026-05-29"), true),
        ],
    )];

    assert_eq!(
        classify(&reports),
        SuitePresence::Ambiguous,
        "Dylint refuses to choose between two suites"
    );
}

/// Count a directory listed twice as the one library it holds.
#[test]
fn a_repeated_directory_counts_once() {
    let directory = "/lints";
    let name = staged_name("nightly-2026-05-28");
    let listing = || vec![listed(directory, &name, true)];
    let reports = [
        directory_report(directory, listing()),
        directory_report(directory, listing()),
    ];

    assert_eq!(
        classify(&reports),
        SuitePresence::ExactlyOne,
        "Dylint deduplicates discovered libraries, so a repeated directory is not ambiguous"
    );
}

/// Keep two suites ambiguous even when a directory is listed twice.
#[test]
fn a_repeated_directory_does_not_hide_two_suites() {
    let directory = "/lints";
    let listing = || {
        vec![
            listed(directory, &staged_name("nightly-2026-05-28"), true),
            listed(directory, &staged_name("nightly-2026-05-29"), true),
        ]
    };
    let reports = [
        directory_report(directory, listing()),
        directory_report(directory, listing()),
    ];

    assert_eq!(
        classify(&reports),
        SuitePresence::Ambiguous,
        "two distinct suites are ambiguous however often their directory is listed"
    );
}

/// Keep the path an exactly-one classification approved.
#[test]
fn exactly_one_suite_keeps_the_path_it_judged() {
    let path = OsString::from("/lints");

    let prepared = DirectLibraryPath::new(path.clone(), SuitePresence::ExactlyOne)
        .expect("exactly one loadable suite makes the path usable");

    assert_eq!(
        prepared.as_os_str(),
        path,
        "the prepared value must keep the path it judged"
    );
}

/// Refuse every classification other than exactly one.
#[rstest]
#[case::absent(SuitePresence::Absent, CheckPreflightError::UserPathHasNoSuite)]
#[case::ambiguous(SuitePresence::Ambiguous, CheckPreflightError::UserPathAmbiguous)]
#[case::unusable(
    SuitePresence::Unusable { path: "/lints".to_owned(), reason: NOT_A_DIRECTORY },
    CheckPreflightError::UserPathUnusable { path: "/lints".to_owned(), reason: NOT_A_DIRECTORY }
)]
fn a_classification_other_than_exactly_one_is_refused(
    #[case] presence: SuitePresence,
    #[case] expected: CheckPreflightError,
) {
    let refused = DirectLibraryPath::new(OsString::from("/lints"), presence);

    assert_eq!(
        refused,
        Err(expected),
        "only an exactly-one classification may become a prepared value"
    );
}
