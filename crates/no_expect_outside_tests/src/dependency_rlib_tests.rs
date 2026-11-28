//! Coverage for dependency artefact selection and related test fixtures.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use rstest::{fixture, rstest};

use super::dependency_rlib;

/// A temporary directory that is removed automatically when dropped, with the
/// capability root its fixtures are made through.
#[derive(Debug)]
struct TemporaryDirectory {
    // The `Dir` is declared first so it drops first: Windows cannot remove a
    // directory while a handle to it is open.
    root: whitaker_common::test_support::fixture_fs::Dir,
    directory: tempfile::TempDir,
}

/// Describes a single fixture artefact: its filename and the
/// seconds-since-Unix-epoch modification timestamp to assign to it.
#[derive(Clone, Copy, Debug)]
struct ArtefactSpec<'a> {
    file_name: &'a str,
    seconds_since_epoch: u64,
}

impl<'a> ArtefactSpec<'a> {
    /// Creates a new spec with the given filename and modification timestamp.
    const fn new(file_name: &'a str, seconds_since_epoch: u64) -> Self {
        Self {
            file_name,
            seconds_since_epoch,
        }
    }
}

/// Holds the outcome of a `dependency_rlib` selection exercise: the directory
/// (kept alive so it is not dropped early), the path that was expected to be
/// chosen, and the path that `dependency_rlib` actually selected.
#[derive(Debug)]
struct DependencyRlibSelection {
    _directory: TemporaryDirectory,
    expected: PathBuf,
    selected: PathBuf,
}

const NEWEST_ARTEFACTS: [ArtefactSpec<'static>; 2] = [
    ArtefactSpec::new("libtokio-older.rlib", 10),
    ArtefactSpec::new("libtokio-newer.rlib", 20),
];
const TIED_ARTEFACTS: [ArtefactSpec<'static>; 2] = [
    ArtefactSpec::new("libtokio-alpha.rlib", 30),
    ArtefactSpec::new("libtokio-zulu.rlib", 30),
];

/// rstest fixture that creates a uniquely named temporary directory for a
/// selection test.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn selection_directory() -> TemporaryDirectory { TemporaryDirectory::new("selection") }

/// Creates `artefacts` inside `directory`, sets their modification times, then
/// invokes `dependency_rlib` and returns both the expected and selected paths
/// for the caller to compare.
fn resolve_dependency_rlib_selection(
    directory: TemporaryDirectory,
    artefacts: &[ArtefactSpec<'_>],
    expected_file_name: &str,
) -> DependencyRlibSelection {
    for artefact in artefacts {
        let path = directory.create_rlib(artefact.file_name);
        set_modified_time(&path, artefact.seconds_since_epoch);
    }

    let expected = directory.path().join(expected_file_name);
    let selected = dependency_rlib(directory.path(), "tokio")
        .expect("Tokio artefact should resolve from fixture directory");

    DependencyRlibSelection {
        _directory: directory,
        expected,
        selected,
    }
}

#[rstest]
#[case::newest(&NEWEST_ARTEFACTS, "libtokio-newer.rlib")]
#[case::ties(&TIED_ARTEFACTS, "libtokio-alpha.rlib")]
fn dependency_rlib_selects_expected_artefact(
    selection_directory: TemporaryDirectory,
    #[case] artefacts: &[ArtefactSpec<'_>],
    #[case] expected_file_name: &str,
) {
    let selection =
        resolve_dependency_rlib_selection(selection_directory, artefacts, expected_file_name);
    assert_eq!(selection.selected, selection.expected);
}

#[rstest]
fn dependency_rlib_returns_err_when_no_matching_rlib_present(
    selection_directory: TemporaryDirectory,
) {
    let error = dependency_rlib(selection_directory.path(), "tokio")
        .expect_err("missing Tokio artefacts should fail to resolve");

    assert!(
        error.contains("failed to locate `tokio` rlib in dependency directory"),
        "unexpected error message, got: {error}",
    );
}

impl TemporaryDirectory {
    /// Creates a new uniquely named temporary directory under the system temp
    /// path and opens its capability root once.
    fn new(name: &str) -> Self {
        let directory = tempfile::Builder::new()
            .prefix(&format!("no-expect-outside-tests-{name}-"))
            .tempdir()
            .expect("temporary directory should be created");
        let root = whitaker_common::test_support::fixture_fs::open_root(directory.path())
            .expect("temporary directory should open as a capability root");
        Self { root, directory }
    }

    /// Returns the path to the temporary directory.
    fn path(&self) -> &Path { self.directory.path() }

    /// Creates an empty `.rlib` fixture file named `file_name` and returns its
    /// path.
    fn create_rlib(&self, file_name: &str) -> PathBuf {
        self.root
            .create(file_name)
            .expect("rlib fixture should be created");
        self.path().join(file_name)
    }
}

/// Sets the last-modified time of `path` to `seconds_since_epoch` seconds
/// after the Unix epoch.
fn set_modified_time(path: &Path, seconds_since_epoch: u64) {
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch);
    filetime::set_file_mtime(path, filetime::FileTime::from_system_time(modified))
        .expect("rlib fixture modified time should be set");
}
