//! Tests that an explicit `DYLINT_LIBRARY_PATH` stays authoritative.

use std::ffi::{OsStr, OsString};

use tempfile::TempDir;

use super::{
    LibraryPath,
    LibraryPathKind,
    active_library_path,
    active_library_path_with,
    tests::{TempBaseDirs, utf8_path, write_record},
};

#[test]
fn explicit_environment_path_precedes_recorded_and_default_paths() {
    let expected = OsString::from("/custom/dylint/lib");

    let actual = active_library_path_with(Some(expected.clone()), None)
        .expect("an explicit path never reads the record");
    assert_eq!(
        actual,
        LibraryPath::Explicit(expected),
        "an explicit environment path should be returned unchanged"
    );
    assert_eq!(
        actual.kind(),
        LibraryPathKind::Explicit,
        "an explicit path should report the explicit kind"
    );
}

/// Check that an explicit path wins over an installation record, even one that
/// would fail to load, so the record is never consulted.
#[test]
fn an_explicit_path_wins_over_a_conflicting_installation_record() {
    let temporary = TempDir::new().expect("temporary directories should be created");
    let data_home = utf8_path(temporary.path());
    write_record(&data_home, b"not JSON");
    let dirs = TempBaseDirs::new(&temporary);
    let explicit = OsString::from("/custom/dylint/lib");

    let actual = active_library_path_with(Some(explicit.clone()), Some(&dirs))
        .expect("an explicit path never reads the record");

    assert_eq!(
        actual,
        LibraryPath::Explicit(explicit),
        "the explicit path should win without the record being read"
    );
}

/// Check that the entry point keeps the value the caller injected.
///
/// The composition root snapshots `DYLINT_LIBRARY_PATH` once per invocation and
/// passes it here, so the resolution never reads the process environment
/// itself; the value it was given is the one that becomes authoritative.
#[test]
fn active_library_path_keeps_the_injected_environment_value() {
    let actual = active_library_path(None, Some(OsStr::new("/inherited/dylint/lib")))
        .expect("an explicit path never reads the record");

    assert_eq!(
        actual,
        LibraryPath::Explicit(OsString::from("/inherited/dylint/lib")),
        "the injected DYLINT_LIBRARY_PATH should be the explicit path"
    );
}

/// Check that an unset or empty `DYLINT_LIBRARY_PATH` is not authoritative.
#[test]
fn an_empty_environment_path_is_not_authoritative() {
    assert_eq!(
        super::explicit_library_path(Some(OsString::new())),
        None,
        "an empty value should be ignored"
    );
    assert_eq!(
        super::explicit_library_path(None),
        None,
        "an unset variable should be ignored"
    );
}
