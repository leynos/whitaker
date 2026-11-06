//! Tests for adapter path selection and child-process behaviour.

use std::{ffi::OsString, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;
use whitaker_common::test_support::with_env_var;
use whitaker_installer::{dirs::BaseDirs, installation_record::InstallationRecordError};

use super::{active_library_path, active_library_path_for, is_installed, recorded_library_paths};
use crate::ports::CliError;

fn utf8_path(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .expect("temporary test paths should be valid UTF-8")
}

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

#[test]
fn explicit_library_path_wins_when_build_and_prebuilt_directories_exist() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let build_path = create_test_directory(&temporary, "build");
    let prebuilt_path = create_test_directory(&temporary, "prebuilt");
    let recorded_path = create_test_directory(&temporary, "recorded");
    let expected = OsString::from("/custom/dylint/lib");
    let actual = active_library_path_for(
        Some(expected.clone()),
        vec![recorded_path],
        Some(build_path),
        Some(prebuilt_path),
    );

    assert_eq!(actual, Some(expected));
}

#[test]
fn explicit_environment_path_precedes_recorded_and_default_paths() {
    let expected = OsString::from("/custom/dylint/lib");

    with_env_var("DYLINT_LIBRARY_PATH", &expected, || {
        let actual = active_library_path(None).expect("an explicit path never reads the record");
        assert_eq!(actual, Some(expected.clone()));
    });
}

/// A [`BaseDirs`] implementation rooted at a temporary directory.
///
/// The record tests need a data directory they own. Redirecting the
/// platform directory through an environment variable works on Unix only,
/// so the directory is injected instead and the tests behave the same on
/// every platform. The default `bootstrap_dir` places the bootstrap beside
/// `whitaker/`, mirroring the real layout.
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
fn absent_bootstrap_metadata_reports_no_installation() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let dirs = TempBaseDirs::new(&temporary);

    assert!(
        !is_installed(&dirs),
        "no bootstrap.json means there is nothing to run from"
    );
}

#[test]
fn recorded_bootstrap_metadata_reports_an_installation() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    // The metadata's contents are irrelevant here: the selection asks only
    // whether a record exists, and a broken record must still be reported
    // by the bootstrap rather than silently bypassed.
    let bootstrap_dir = data_home.join("whitaker-bootstrap");
    std::fs::create_dir_all(&bootstrap_dir)
        .expect("bootstrap directory should be created inside the temporary directory");
    std::fs::write(bootstrap_dir.join("bootstrap.json"), b"{}")
        .expect("bootstrap metadata should be written");
    let dirs = TempBaseDirs::new(&temporary);

    assert!(is_installed(&dirs), "a recorded bootstrap must be used");
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

    let actual = active_library_path_for(None, Vec::new(), Some(build_path), Some(prebuilt_path));

    assert_eq!(actual, Some(expected));
}

#[test]
fn prebuilt_directory_is_used_when_build_directory_is_absent() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let missing_build_path = utf8_path(&temporary.path().join("build"));
    let prebuilt_path = create_test_directory(&temporary, "prebuilt");
    let expected = prebuilt_path.clone().into_os_string();

    let actual = active_library_path_for(
        None,
        Vec::new(),
        Some(missing_build_path),
        Some(prebuilt_path),
    );

    assert_eq!(actual, Some(expected));
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

    let actual = active_library_path_for(None, vec![stable, nightly], Some(active_build), None);

    assert_eq!(actual, Some(expected));
}

#[cfg(unix)]
#[test]
fn an_unrepresentable_recorded_directory_does_not_hide_valid_paths() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let valid = create_test_directory(&temporary, "valid");
    let invalid = create_test_directory(&temporary, "invalid:directory");
    let expected = valid.clone().into_os_string();

    let actual = active_library_path_for(None, vec![invalid, valid], None, None);

    assert_eq!(actual, Some(expected));
}
