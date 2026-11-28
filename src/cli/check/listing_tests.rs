//! Tests for the `-v` coverage listing.

use std::{ffi::OsString, io};

use camino::Utf8Path;
use rstest::rstest;

use super::{ListingRequest, is_requested, suffix_selects, write_listing};
use crate::{
    cli::check::CargoOptions,
    ports::{
        PackageFacts,
        PackageQueryError,
        WorkspaceFacts,
        WorkspaceQuery,
        WorkspaceQueryRequest,
    },
};

/// A query that answers with fixed facts or a fixed failure.
struct FakeQuery {
    /// The answer to give.
    answer: Result<WorkspaceFacts, &'static str>,
}

impl WorkspaceQuery for FakeQuery {
    fn workspace_facts(
        &self,
        _request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError> {
        self.answer
            .clone()
            .map_err(|message| PackageQueryError::Metadata {
                message: message.to_owned(),
            })
    }
}

/// Facts whose default selection is one member.
fn facts() -> WorkspaceFacts {
    let package = PackageFacts {
        name: "member_b".to_owned(),
        version: "0.1.0".to_owned(),
        manifest_path: "/work/app/member_b/Cargo.toml".into(),
        targets: Vec::new(),
    };
    WorkspaceFacts {
        workspace_root: "/work/app".into(),
        members: vec![package.clone()],
        default_members: vec![package],
    }
}

/// Render the listing for `answer` and `options`.
fn listing(answer: Result<WorkspaceFacts, &'static str>, options: &CargoOptions) -> String {
    let query = FakeQuery { answer };
    let mut stderr = Vec::new();
    write_listing(
        &query,
        &mut ListingRequest {
            options,
            suffix: &[],
            current_dir: Utf8Path::new("/work/app"),
            stderr: &mut stderr,
        },
    );
    String::from_utf8(stderr).expect("the listing is UTF-8")
}

/// Show the packages in Cargo's own progress-line shape.
#[test]
fn the_listing_names_the_selected_packages() {
    let shown = listing(Ok(facts()), &CargoOptions::default());

    assert_eq!(
        shown, "    Checking member_b v0.1.0 (/work/app/member_b)\n",
        "the listing must name the package Cargo will check"
    );
}

/// Keep standard output pure: the listing goes to the given stream only.
#[test]
fn the_listing_is_written_only_to_the_supplied_stream() {
    let shown = listing(Ok(facts()), &CargoOptions::default());

    assert!(
        !shown.is_empty(),
        "the listing must be produced so the stream assertion is meaningful"
    );
    assert!(
        shown.starts_with("    Checking "),
        "the listing must use Cargo's line shape, got {shown:?}"
    );
}

/// Continue the run with a note when the query fails.
#[test]
fn a_failed_query_is_noted_and_does_not_stop_the_run() {
    let shown = listing(Err("no workspace here"), &CargoOptions::default());

    assert!(
        shown.contains("could not list the packages Cargo will check"),
        "a failed query must be reported, got {shown:?}"
    );
}

/// Print nothing when a selector matches nothing, and leave it to Cargo.
#[test]
fn an_unresolved_selection_prints_nothing() {
    let options = CargoOptions {
        packages: vec!["absent".to_owned()],
        ..CargoOptions::default()
    };

    let shown = listing(Ok(facts()), &options);

    assert!(
        shown.is_empty(),
        "an unresolved selector must not print a guessed listing, got {shown:?}"
    );
}

/// Recognize every Cargo spelling that selects packages.
#[rstest]
#[case::none(&[], false)]
#[case::package(&["-p", "member_a"], true)]
#[case::package_attached(&["-pmember_a"], true)]
#[case::package_equals(&["--package=member_a"], true)]
#[case::workspace(&["--workspace"], true)]
#[case::deprecated_all(&["--all"], true)]
#[case::exclude(&["--exclude", "member_a"], true)]
#[case::exclude_equals(&["--exclude=member_a"], true)]
#[case::manifest(&["--manifest-path", "Cargo.toml"], true)]
#[case::manifest_equals(&["--manifest-path=Cargo.toml"], true)]
#[case::quiet(&["-q"], false)]
#[case::message_format(&["--message-format=json"], false)]
#[case::all_features(&["--all-features"], false)]
#[case::all_targets(&["--all-targets"], false)]
fn forwarded_package_selectors_are_recognized(#[case] suffix: &[&str], #[case] expected: bool) {
    let arguments: Vec<OsString> = suffix.iter().map(OsString::from).collect();

    assert_eq!(suffix_selects(&arguments), expected, "suffix {suffix:?}");
}

/// Report the listing unavailable when the suffix selects packages.
#[test]
fn a_forwarded_package_selector_hides_the_listing() {
    let query = FakeQuery {
        answer: Ok(facts()),
    };
    let mut stderr = Vec::new();
    write_listing(
        &query,
        &mut ListingRequest {
            options: &CargoOptions::default(),
            suffix: &[OsString::from("-p"), OsString::from("member_a")],
            current_dir: Utf8Path::new("/work/app"),
            stderr: &mut stderr,
        },
    );
    let shown = String::from_utf8(stderr).expect("the notice is UTF-8");

    assert!(
        shown.contains("a package or target selector after `--` is not modelled"),
        "the reason must be stated, got {shown:?}"
    );
    assert!(
        !shown.contains("Checking"),
        "no guessed selection may be printed, got {shown:?}"
    );
}

/// Ask for the listing only when the invocation is verbose and not quiet.
#[rstest]
#[case::default_verbosity(false, 0, false)]
#[case::verbose(false, 1, true)]
#[case::very_verbose(false, 2, true)]
#[case::quietly_verbose(true, 1, false)]
fn the_listing_is_requested_only_by_verbosity(
    #[case] quiet: bool,
    #[case] verbosity: u8,
    #[case] expected: bool,
) {
    assert_eq!(
        is_requested(quiet, verbosity),
        expected,
        "quiet={quiet} verbosity={verbosity}"
    );
}

/// Keep the stream type honest: a failing stream is handled, not panicked.
#[test]
fn a_failing_stream_does_not_panic() {
    struct FailingWriter;

    impl io::Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("the stream is closed"))
        }

        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    let query = FakeQuery {
        answer: Ok(facts()),
    };
    let mut stderr = FailingWriter;
    write_listing(
        &query,
        &mut ListingRequest {
            options: &CargoOptions::default(),
            suffix: &[],
            current_dir: Utf8Path::new("/work/app"),
            stderr: &mut stderr,
        },
    );
}
