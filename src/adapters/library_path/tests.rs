//! Tests for library-path resolution and installation-record handling.

use std::{ffi::OsString, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;
use whitaker_common::test_support::with_env_var;
use whitaker_installer::{
    dependency_binaries::host_target,
    dirs::BaseDirs,
    installation_record::InstallationRecordError,
};

use super::{
    LibraryPath,
    LibraryPathKind,
    active_library_path,
    prebuilt_library_path,
    recorded_library_paths,
    resolve_library_path,
};
use crate::ports::CliError;

/// Convert a temporary path to the UTF-8 form the installer uses.
fn utf8_path(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .expect("temporary test paths should be valid UTF-8")
}

/// Create `name` inside `temporary` and return its UTF-8 path.
fn create_test_directory(temporary: &TempDir, name: &str) -> Utf8PathBuf {
    let root = Dir::open_ambient_dir(
        Utf8Path::from_path(temporary.path()).expect("temporary path should be valid UTF-8"),
        ambient_authority(),
    )
    .expect("temporary directory should be opened as a filesystem capability");
    root.create_dir_all(name)
        .expect("test directory should be created inside the temporary directory");

    utf8_path(&temporary.path().join(name))
}

/// A [`BaseDirs`] implementation rooted at a temporary directory.
///
/// The record tests need a data directory they own. Redirecting the
/// platform directory through an environment variable works on Unix only,
/// so the directory is injected instead and the tests behave the same on
/// every platform.
struct TempBaseDirs {
    root: PathBuf,
}

impl TempBaseDirs {
    /// Anchor the platform directories inside `temporary`'s tree.
    fn new(temporary: &TempDir) -> Self {
        Self {
            root: temporary.path().to_path_buf(),
        }
    }
}

impl BaseDirs for TempBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.root.join("whitaker")) }
}

/// A [`BaseDirs`] with no platform directories at all.
struct AbsentBaseDirs;

impl BaseDirs for AbsentBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { None }
}

/// Write `contents` as the installation record under `data_home`.
fn write_record(data_home: &Utf8Path, contents: &[u8]) -> Utf8PathBuf {
    let record_dir = data_home.join("whitaker");
    std::fs::create_dir_all(&record_dir)
        .expect("record directory should be created inside the temporary directory");
    let record_path = record_dir.join("installed_lint_paths.json");
    std::fs::write(&record_path, contents).expect("record should be written");
    record_path
}

#[test]
fn explicit_environment_path_precedes_recorded_and_default_paths() {
    let expected = OsString::from("/custom/dylint/lib");

    with_env_var("DYLINT_LIBRARY_PATH", &expected, || {
        let actual = active_library_path(None).expect("an explicit path never reads the record");
        assert_eq!(actual, LibraryPath::Explicit(expected.clone()));
        assert_eq!(actual.kind(), LibraryPathKind::Explicit);
    });
}

#[test]
fn malformed_installation_record_is_reported() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    write_record(&data_home, b"not JSON");
    let dirs = TempBaseDirs::new(&temporary);

    let error =
        recorded_library_paths(Some(&dirs)).expect_err("a malformed record must not be ignored");
    assert!(
        matches!(
            error,
            CliError::InstallationRecord {
                source: InstallationRecordError::Parse { .. }
            }
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn unreadable_installation_record_is_reported() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    // A directory in place of the record opens but cannot be read, which
    // fails the same way an unreadable file does without depending on the
    // test runner's privileges.
    std::fs::create_dir_all(data_home.join("whitaker/installed_lint_paths.json"))
        .expect("a directory should stand in for the unreadable record");
    let dirs = TempBaseDirs::new(&temporary);

    let error =
        recorded_library_paths(Some(&dirs)).expect_err("an unreadable record must not be ignored");
    assert!(
        matches!(
            error,
            CliError::InstallationRecord {
                source: InstallationRecordError::Io { .. }
            }
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn absent_directory_provider_yields_no_recorded_paths() {
    let recorded = recorded_library_paths(None).expect("no provider is not a failure");

    assert!(recorded.is_empty());
}

#[test]
fn absent_installation_record_yields_no_recorded_paths() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::new(&temporary);

    let recorded = recorded_library_paths(Some(&dirs)).expect("a missing record is not a failure");
    assert!(recorded.is_empty());
}

#[test]
fn build_directory_precedes_prebuilt_directory_in_candidate_list() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let build_path = create_test_directory(&temporary, "build");
    let prebuilt_path = create_test_directory(&temporary, "prebuilt");
    let expected = std::env::join_paths([
        build_path.as_std_path().to_path_buf(),
        prebuilt_path.as_std_path().to_path_buf(),
    ])
    .expect("temporary paths should form an environment path list");

    let actual = resolve_library_path(Vec::new(), Some(build_path), Some(prebuilt_path))
        .expect("existing directories should join");

    assert_eq!(
        actual,
        LibraryPath::Resolved {
            path: expected,
            candidates: 2,
        }
    );
    assert_eq!(actual.kind(), LibraryPathKind::Resolved);
    assert_eq!(actual.candidate_count(), 2);
}

#[test]
fn prebuilt_directory_is_used_when_build_directory_is_absent() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let missing_build_path = utf8_path(&temporary.path().join("build"));
    let prebuilt_path = create_test_directory(&temporary, "prebuilt");
    let expected = prebuilt_path.clone().into_os_string();

    let actual = resolve_library_path(Vec::new(), Some(missing_build_path), Some(prebuilt_path))
        .expect("an existing directory should join");

    assert_eq!(
        actual,
        LibraryPath::Resolved {
            path: expected,
            candidates: 1,
        }
    );
}

#[test]
fn recorded_and_default_directories_cover_each_toolchain() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let stable = create_test_directory(&temporary, "stable-custom-target");
    let nightly = create_test_directory(&temporary, "nightly-custom-target");
    let active_build = create_test_directory(&temporary, "active-default-target");
    let expected = std::env::join_paths([
        stable.as_std_path().to_path_buf(),
        nightly.as_std_path().to_path_buf(),
        active_build.as_std_path().to_path_buf(),
    ])
    .expect("temporary paths should form an environment path list");

    let actual = resolve_library_path(vec![stable, nightly], Some(active_build), None)
        .expect("existing directories should join");

    assert_eq!(
        actual,
        LibraryPath::Resolved {
            path: expected,
            candidates: 3,
        }
    );
}

#[test]
fn no_existing_directory_is_a_normal_no_candidate_result() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let missing = utf8_path(&temporary.path().join("missing"));

    let actual = resolve_library_path(vec![missing.clone()], Some(missing), None)
        .expect("a missing directory is not a failure");

    assert_eq!(actual, LibraryPath::NoCandidate);
    assert_eq!(actual.kind(), LibraryPathKind::NoCandidate);
    assert_eq!(actual.as_deref(), None);
}

#[cfg(unix)]
#[test]
fn an_unrepresentable_recorded_directory_does_not_hide_valid_paths() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let valid = create_test_directory(&temporary, "valid");
    let invalid = create_test_directory(&temporary, "invalid:directory");
    let expected = valid.clone().into_os_string();

    let actual = resolve_library_path(vec![invalid, valid], None, None)
        .expect("the surviving directory should join");

    assert_eq!(
        actual,
        LibraryPath::Resolved {
            path: expected,
            candidates: 1,
        }
    );
}

#[cfg(unix)]
#[test]
fn an_unusable_candidate_with_no_survivors_is_reported() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let invalid = create_test_directory(&temporary, "invalid:directory");

    let error = resolve_library_path(vec![invalid], None, None)
        .expect_err("an unusable candidate must not look like nothing installed");

    assert!(
        matches!(error, CliError::LibraryPathUnusable { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn a_candidate_that_is_no_longer_a_directory_is_stale_not_broken() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let file = utf8_path(&temporary.path().join("recorded-file"));
    std::fs::write(&file, b"not a directory").expect("the file should be written");

    let actual = resolve_library_path(vec![file], None, None)
        .expect("a path that is no longer a directory is a stale record");

    assert_eq!(actual, LibraryPath::NoCandidate);
}

#[test]
fn prebuilt_library_path_reports_an_unavailable_data_directory() {
    if host_target().is_none() {
        // A host with no recognized target triple has no prebuilt candidate,
        // so there is no directory call to fail.
        return;
    }

    let error = prebuilt_library_path(Some(&AbsentBaseDirs), "nightly-test")
        .expect_err("an absent data directory must be reported");

    assert!(
        matches!(error, CliError::PrebuiltLibraryPath { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn prebuilt_library_path_is_absent_without_a_directory_provider() {
    let resolved =
        prebuilt_library_path(None, "nightly-test").expect("no provider is not a failure");

    assert_eq!(resolved, None);
}
