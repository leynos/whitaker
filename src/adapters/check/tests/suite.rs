//! Which library paths the adapter will let Dylint load the suite from.

use std::ffi::{OsStr, OsString};

use rstest::rstest;
use tempfile::TempDir;

use super::{
    super::{
        LibraryMode,
        mode_for,
        preflight_user_managed_in,
        suite::{recorded_suite_present, suite_present},
    },
    fixtures::{
        directory_with_suite,
        other_library_filename,
        other_suite_filename,
        path_list,
        suite_filename,
        temp_dirs,
    },
};
use crate::{
    library_path::{NOT_A_DIRECTORY, NOT_A_LIBRARY, has_suite_name},
    ports::{CheckPreflightError, CheckPrepared},
};

/// Accept a path whose directory holds the staged suite.
#[test]
fn a_directory_holding_the_suite_satisfies_the_path() {
    let directory = directory_with_suite();

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
    let holding = directory_with_suite();

    let result = suite_present(&path_list(&[&empty, &holding]));

    assert!(
        result.is_ok(),
        "a later directory must still be searched, got {result:?}"
    );
}

/// Refuse a list with a directory that no longer exists.
///
/// Dylint 6.3.0 requires every entry to name an existing directory and rejects
/// the whole list otherwise, so a usable entry elsewhere must not mask the
/// defect. The holding directory is first on the list on purpose: skipping the
/// missing entry would pass a test whose only entry is absent, but not this one.
#[test]
fn a_missing_directory_is_refused_rather_than_skipped() {
    let holding = directory_with_suite();
    let missing = TempDir::new().expect("a temporary directory is available");
    let absent = missing.path().join("removed-by-a-later-uninstall");
    let path = std::env::join_paths([holding.path().as_os_str(), absent.as_os_str()])
        .expect("temporary paths are representable");

    let result = suite_present(&path);

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathUnusable {
            path: absent.display().to_string(),
            reason: NOT_A_DIRECTORY,
        }),
        "Dylint rejects a list entry that is not a directory, so check must refuse first"
    );
}

/// Report an unreadable directory as a host fault rather than a missing suite.
///
/// The permission bits are the only way to make a directory that exists fail to
/// read on every platform, and a root-owned process reads through them. The
/// test therefore skips itself where the host cannot express the failure, as
/// the capability, not the code, is what is missing.
#[cfg(unix)]
#[test]
fn an_unreadable_directory_is_a_host_fault() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TempDir::new().expect("a temporary directory is available");
    let unreadable = directory.path().join("unreadable");
    std::fs::create_dir(&unreadable).expect("the directory can be created");
    let restore = || {
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o755))
            .expect("the directory permissions can be restored");
    };
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
        .expect("the directory permissions can be removed");

    if std::fs::read_dir(&unreadable).is_ok() {
        restore();
        return;
    }
    let result = suite_present(&OsString::from(unreadable.as_os_str()));
    restore();

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
        mode_for(explicit.as_deref()),
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

/// Approve a recorded directory that names exactly one suite.
///
/// The installation records one directory per toolchain and target, so this is
/// the shape a real record has. The run re-check calls the same function, so
/// approval here is approval there.
#[test]
fn a_recorded_directory_with_one_suite_is_approved() {
    let directory = directory_with_suite();

    let result = recorded_suite_present(directory.path().as_os_str());

    assert!(
        result.is_ok(),
        "one staged suite must be approved, got {result:?}"
    );
}

/// Report a recorded directory that lost the suite as missing.
#[test]
fn a_recorded_directory_without_the_suite_is_missing() {
    let directory = TempDir::new().expect("a temporary directory is available");

    let result = recorded_suite_present(directory.path().as_os_str());

    assert!(
        matches!(
            result,
            Err(CheckPreflightError::Bootstrap(
                whitaker_installer::bootstrap::BootstrapError::LibrariesMissing { .. }
            ))
        ),
        "a directory holding no suite is an incomplete installation, got {result:?}"
    );
}

/// Refuse a recorded directory that names two suites.
///
/// Dylint 6.3.0 exits `1` without linting when `--lib=whitaker_suite` matches
/// two libraries, and that status is indistinguishable from a lint denial. The
/// recorded directory is therefore held to the same exactly-one rule as the
/// user's path, and the run re-check calls this function too, so a second
/// suite appearing after readiness is also refused instead of being passed to
/// Dylint.
#[test]
fn a_recorded_directory_with_two_suites_is_ambiguous() {
    let directory = TempDir::new().expect("a temporary directory is available");
    for name in [suite_filename(), other_suite_filename()] {
        std::fs::write(directory.path().join(name), b"").expect("the suite file can be created");
    }
    let path = directory.path().as_os_str();

    let result = recorded_suite_present(path);

    assert_eq!(
        result,
        Err(CheckPreflightError::RecordedPathAmbiguous {
            path: path.to_string_lossy().into_owned(),
        }),
        "a recorded directory Dylint cannot choose within must be refused before the run"
    );
}

/// Report the host fault when the data directory cannot be determined.
///
/// Without one there is no way to tell whether an installation is recorded, so
/// the user-managed route must report the same host fault the recorded route
/// does rather than silently running a path the record might contradict.
#[test]
fn a_user_managed_path_without_a_data_directory_is_a_host_fault() {
    let directory = directory_with_suite();

    let result = preflight_user_managed_in(None, directory.path().as_os_str(), None);

    assert!(
        matches!(
            result,
            Err(CheckPreflightError::Bootstrap(
                whitaker_installer::bootstrap::BootstrapError::NoDataDirectory
            ))
        ),
        "an undeterminable data directory is a host fault, got {result:?}"
    );
}

/// Run the user's own path directly when nothing has been recorded.
#[test]
fn a_user_managed_path_with_no_record_runs_directly() {
    let (_root, dirs) = temp_dirs();
    let library = directory_with_suite();

    let result = preflight_user_managed_in(Some(&dirs), library.path().as_os_str(), None);

    assert!(
        matches!(result, Ok(CheckPrepared::Direct(_))),
        "with no record the user's path is the whole installation, got {result:?}"
    );
}

/// Refuse a directory that merely carries the suite's file name.
///
/// Dylint discovers candidates by file name, so this entry reaches its
/// selection and then fails to load, with the status a lint denial uses. It is
/// therefore a provisioning defect, reported as unusable rather than absent.
#[test]
fn a_directory_named_like_the_suite_is_not_the_suite() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let suite = directory.path().join(suite_filename());
    std::fs::create_dir(&suite).expect("the suite-named directory can be created");

    let result = suite_present(&path_list(&[&directory]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathUnusable {
            path: suite.display().to_string(),
            reason: NOT_A_LIBRARY,
        }),
        "a directory is not a library Dylint could load"
    );
}

/// Count a directory listed twice as the one suite it holds.
///
/// Dylint stores discovered libraries in a set, so the same directory twice is
/// one candidate. Refusing the repetition would reject a path Dylint accepts.
#[test]
fn a_repeated_directory_counts_once() {
    let directory = directory_with_suite();
    let repeated = std::env::join_paths([directory.path(), directory.path()])
        .expect("temporary paths are representable");

    let result = suite_present(&repeated);

    assert!(
        result.is_ok(),
        "a repeated directory holds one library, not two, got {result:?}"
    );
}

/// Refuse a recorded directory whose single candidate cannot be loaded.
///
/// The record is Whitaker's own, so the refusal is reported as a record defect:
/// reinstalling is the remedy, and the message must not send the user looking
/// at a path they never set.
#[test]
fn a_recorded_suite_named_directory_is_unusable() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let suite = directory.path().join(suite_filename());
    std::fs::create_dir(&suite).expect("the suite-named directory can be created");

    let result = recorded_suite_present(directory.path().as_os_str());

    assert_eq!(
        result,
        Err(CheckPreflightError::RecordedPathUnusable {
            path: suite.display().to_string(),
            reason: NOT_A_LIBRARY,
        }),
        "a record naming a directory instead of the suite is a provisioning defect"
    );
}
