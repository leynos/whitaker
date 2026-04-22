//! Coverage for dependency artefact selection and related test fixtures.

use super::dependency_rlib;
use rstest::{fixture, rstest};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A temporary directory that is removed automatically when dropped.
#[derive(Debug)]
struct TemporaryDirectory(PathBuf);

/// Describes a single fixture artefact: its filename and the
/// seconds-since-Unix-epoch modification timestamp to assign to it.
#[derive(Clone, Copy, Debug)]
struct ArtifactSpec<'a> {
    file_name: &'a str,
    seconds_since_epoch: u64,
}

impl<'a> ArtifactSpec<'a> {
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

const NEWEST_ARTIFACTS: [ArtifactSpec<'static>; 2] = [
    ArtifactSpec::new("libtokio-older.rlib", 10),
    ArtifactSpec::new("libtokio-newer.rlib", 20),
];
const TIED_ARTIFACTS: [ArtifactSpec<'static>; 2] = [
    ArtifactSpec::new("libtokio-alpha.rlib", 30),
    ArtifactSpec::new("libtokio-zulu.rlib", 30),
];

/// rstest fixture that creates a uniquely named temporary directory for a
/// selection test.
#[fixture]
fn selection_directory() -> TemporaryDirectory {
    TemporaryDirectory::new("selection")
}

/// Creates `artifacts` inside `directory`, sets their modification times, then
/// invokes `dependency_rlib` and returns both the expected and selected paths
/// for the caller to compare.
fn resolve_dependency_rlib_selection(
    directory: TemporaryDirectory,
    artifacts: &[ArtifactSpec<'_>],
    expected_file_name: &str,
) -> DependencyRlibSelection {
    for artifact in artifacts {
        let path = create_rlib(directory.path(), artifact.file_name);
        set_modified_time(&path, artifact.seconds_since_epoch);
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
#[case("newest", &NEWEST_ARTIFACTS, "libtokio-newer.rlib")]
#[case("ties", &TIED_ARTIFACTS, "libtokio-alpha.rlib")]
fn dependency_rlib_selects_expected_artifact(
    selection_directory: TemporaryDirectory,
    #[case] _directory_name: &str,
    #[case] artifacts: &[ArtifactSpec<'_>],
    #[case] expected_file_name: &str,
) {
    let selection =
        resolve_dependency_rlib_selection(selection_directory, artifacts, expected_file_name);
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
    /// path.
    fn new(name: &str) -> Self {
        let unique = format!(
            "no-expect-outside-tests-{name}-{}",
            std::time::UNIX_EPOCH
                .elapsed()
                .expect("clock should be after the Unix epoch")
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&directory).expect("temporary directory should be created");
        Self(directory)
    }

    /// Returns the path to the temporary directory.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Creates an empty `.rlib` fixture file at `directory/file_name` and returns
/// its path.
fn create_rlib(directory: &Path, file_name: &str) -> PathBuf {
    let path = directory.join(file_name);
    File::create(&path).expect("rlib fixture should be created");
    path
}

/// Sets the last-modified time of `path` to `seconds_since_epoch` seconds
/// after the Unix epoch.
fn set_modified_time(path: &Path, seconds_since_epoch: u64) {
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch);
    let file = File::options()
        .write(true)
        .open(path)
        .expect("rlib fixture should be reopened");
    let existing_accessed = file
        .metadata()
        .expect("rlib fixture metadata should be readable")
        .accessed();
    let times = existing_accessed
        .map(|accessed| std::fs::FileTimes::new().set_accessed(accessed))
        .unwrap_or_else(|_| std::fs::FileTimes::new())
        .set_modified(modified);
    file.set_times(times)
        .expect("rlib fixture modified time should be set");
}
