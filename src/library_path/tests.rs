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

/// One directory entry, given its full path and kind.
fn listed(path: PathBuf, is_regular_file: bool) -> ListedFile {
    ListedFile {
        path,
        is_regular_file,
    }
}

/// A path-list entry whose directory holds `files`.
fn directory_report(path: &str, files: Vec<ListedFile>) -> EntryReport {
    report(path, EntryListing::Directory(files))
}

/// One entry Dylint cannot search, and the refusal it earns.
struct Unsearchable {
    /// The entry as it appears on the path list.
    entry: &'static str,
    /// What the adapter reported for it.
    listing: EntryListing,
    /// The reason the refusal must carry.
    reason: &'static str,
}

/// Refuse an entry Dylint could not search, naming it and the reason.
///
/// A relative entry is refused before any filesystem question is asked, and an
/// entry that names nothing or names a non-directory is refused as not a
/// directory. All three are the user's to fix, so the refusal must name the
/// entry rather than an opaque verdict.
#[rstest]
#[case::relative(Unsearchable { entry: "lints", listing: EntryListing::Uninspected, reason: NOT_ABSOLUTE })]
#[case::missing(Unsearchable { entry: "/lints", listing: EntryListing::Missing, reason: NOT_A_DIRECTORY })]
#[case::not_a_directory(Unsearchable { entry: "/lints/file", listing: EntryListing::NotADirectory, reason: NOT_A_DIRECTORY })]
fn an_entry_dylint_cannot_search_is_unusable(#[case] case: Unsearchable) {
    let reports = [report(case.entry, case.listing)];

    assert_eq!(
        classify(&reports),
        SuitePresence::Unusable {
            path: case.entry.to_owned(),
            reason: case.reason,
        },
        "the refusal must name the entry it could not search"
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
        vec![listed(PathBuf::from("/lints").join("notes.txt"), true)],
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
        vec![listed(PathBuf::from(directory).join(&name), true)],
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
        vec![listed(PathBuf::from(directory).join(&name), false)],
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
            listed(
                PathBuf::from(directory).join(staged_name("nightly-2026-05-28")),
                true,
            ),
            listed(
                PathBuf::from(directory).join(staged_name("nightly-2026-05-29")),
                true,
            ),
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
    let listing = || vec![listed(PathBuf::from(directory).join(&name), true)];
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
            listed(
                PathBuf::from(directory).join(staged_name("nightly-2026-05-28")),
                true,
            ),
            listed(
                PathBuf::from(directory).join(staged_name("nightly-2026-05-29")),
                true,
            ),
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
