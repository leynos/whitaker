//! Coverage for dependency artefact selection and related test fixtures.

use super::dependency_rlib;
use rstest::{fixture, rstest};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug)]
struct TemporaryDirectory(PathBuf);

#[derive(Clone, Copy, Debug)]
struct ArtifactSpec<'a> {
    file_name: &'a str,
    seconds_since_epoch: u64,
}

impl<'a> ArtifactSpec<'a> {
    const fn new(file_name: &'a str, seconds_since_epoch: u64) -> Self {
        Self {
            file_name,
            seconds_since_epoch,
        }
    }
}

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

#[fixture]
fn selection_directory() -> TemporaryDirectory {
    TemporaryDirectory::new("selection")
}

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

impl TemporaryDirectory {
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

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn create_rlib(directory: &Path, file_name: &str) -> PathBuf {
    let path = directory.join(file_name);
    File::create(&path).expect("rlib fixture should be created");
    path
}

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
