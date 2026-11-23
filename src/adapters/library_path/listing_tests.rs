//! What the read-only seam reports for a real path list.
//!
//! Each case builds the smallest directory shape that can distinguish the
//! listing from a plausible wrong one: a relative entry that must not be read,
//! a missing entry that must not become a host fault, a symlink that must be
//! followed, and a plain directory whose files must be described accurately.

use std::ffi::OsString;

use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;
use whitaker_installer::{crate_name::CrateName, resolution::SUITE_CRATE, stager::Stager};

use super::{EntryKind, classify_entry, inspect};
use crate::library_path::{EntryListing, has_suite_name};

/// The staged name of the Whitaker suite library for `toolchain`.
fn staged_name(toolchain: &str) -> String {
    Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// Join `directories` into one platform path list.
fn path_list(directories: &[&TempDir]) -> OsString {
    std::env::join_paths(directories.iter().map(|directory| directory.path()))
        .expect("temporary paths are representable")
}

/// Report a relative entry as uninspected, with no filesystem question asked.
///
/// The entry names nothing a host could hold, so a seam that tried to read it
/// would report a host fault for what is the user's defect.
#[test]
fn a_relative_entry_is_uninspected() {
    let reports = inspect(&OsString::from("lints")).expect("a relative entry is not a host fault");

    assert_eq!(
        reports.len(),
        1,
        "the whole list is reported, including the entry that was not read"
    );
    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&EntryListing::Uninspected),
        "a relative entry must be reported without reading the filesystem"
    );
}

/// Report an absent entry as missing, not as a host fault.
#[test]
fn a_missing_entry_is_missing() {
    let outer = TempDir::new().expect("a temporary directory is available");
    let absent = outer.path().join("removed-by-a-later-uninstall");
    let path = std::env::join_paths([absent.as_os_str()]).expect("paths are representable");

    let reports = inspect(&path).expect("absence is a description, not a host fault");

    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&EntryListing::Missing),
        "an entry that names nothing is reported as missing"
    );
}

/// Report a file on the list as not a directory.
#[test]
fn a_file_entry_is_not_a_directory() {
    let outer = TempDir::new().expect("a temporary directory is available");
    let file = outer.path().join("not-a-directory");
    std::fs::write(&file, b"").expect("the file can be created");
    let path = std::env::join_paths([file.as_os_str()]).expect("paths are representable");

    let reports = inspect(&path).expect("a file is a description, not a host fault");

    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&EntryListing::NotADirectory),
        "an entry that names a file is not a directory Dylint can search"
    );
}

/// Describe a directory's files and their kind.
///
/// The suite file and the unrelated file must be told apart by name, and the
/// listing must say which resolve to regular files, because the rules decide
/// loadability from that flag rather than from a second metadata read.
#[test]
fn a_directory_reports_its_files_and_their_kind() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let suite = staged_name("nightly-2026-05-28");
    std::fs::write(directory.path().join(&suite), b"").expect("the suite file can be created");
    std::fs::create_dir(directory.path().join("nested"))
        .expect("the nested directory can be created");

    let reports = inspect(&path_list(&[&directory])).expect("the directory can be listed");
    let Some(EntryListing::Directory(files)) = reports.first().map(|report| &report.listing) else {
        panic!("a directory on the list must be listed, got {reports:?}");
    };

    let found = files
        .iter()
        .find(|file| file.path.file_name().is_some_and(has_suite_name));
    let Some(suite_file) = found else {
        panic!("the staged suite file must appear in the listing, got {files:?}");
    };
    assert!(
        suite_file.is_regular_file,
        "a staged suite file resolves to a regular file"
    );
    let nested = files
        .iter()
        .find(|file| file.path.ends_with("nested"))
        .map(|file| file.is_regular_file);
    assert_eq!(
        nested,
        Some(false),
        "a nested directory must not be reported as a regular file"
    );
}

/// Follow a symlink when reporting whether an entry is a regular file.
#[cfg(unix)]
#[test]
fn a_symlink_is_reported_as_the_file_it_resolves_to() {
    let target = TempDir::new().expect("a temporary directory is available");
    let suite = staged_name("nightly-2026-05-28");
    let file = target.path().join(&suite);
    std::fs::write(&file, b"").expect("the suite file can be created");
    let linked = TempDir::new().expect("a temporary directory is available");
    std::os::unix::fs::symlink(&file, linked.path().join(&suite))
        .expect("the symlink can be created");

    let reports = inspect(&path_list(&[&linked])).expect("the directory can be listed");
    let Some(EntryListing::Directory(files)) = reports.first().map(|report| &report.listing) else {
        panic!("a directory on the list must be listed, got {reports:?}");
    };

    assert!(
        files.iter().any(|listed| listed.is_regular_file),
        "a symlink to a regular file is reported as one"
    );
}

/// Report a suite-named directory as not a regular file.
#[test]
fn a_suite_named_directory_is_not_a_regular_file() {
    let directory = TempDir::new().expect("a temporary directory is available");
    let suite = staged_name("nightly-2026-05-28");
    std::fs::create_dir(directory.path().join(&suite))
        .expect("the suite-named directory can be created");

    let reports = inspect(&path_list(&[&directory])).expect("the directory can be listed");
    let Some(EntryListing::Directory(files)) = reports.first().map(|report| &report.listing) else {
        panic!("a directory on the list must be listed, got {reports:?}");
    };

    let suite_file = files
        .iter()
        .find(|file| file.path.file_name().is_some_and(has_suite_name));
    assert_eq!(
        suite_file.map(|file| file.is_regular_file),
        Some(false),
        "a directory carrying the suite's name is not a library file"
    );
}

/// Classify a real entry once, for every caller that asks.
#[rstest]
#[case::directory(true)]
#[case::file(false)]
fn classify_entry_reports_the_shape_on_disk(#[case] is_directory: bool) {
    let outer = TempDir::new().expect("a temporary directory is available");
    let path = outer.path().join(if is_directory { "dir" } else { "file" });
    if is_directory {
        std::fs::create_dir(&path).expect("the directory can be created");
    } else {
        std::fs::write(&path, b"").expect("the file can be created");
    }

    let expected = if is_directory {
        EntryKind::Directory
    } else {
        EntryKind::NotADirectory
    };

    assert_eq!(
        classify_entry(&path),
        Ok(expected),
        "metadata decides the entry's shape, and only that"
    );
}

/// Report a path that names nothing as missing rather than a host fault.
#[test]
fn classify_entry_reports_an_absent_path_as_missing() {
    let outer = TempDir::new().expect("a temporary directory is available");
    let absent = outer.path().join("gone");

    assert_eq!(
        classify_entry(&absent),
        Ok(EntryKind::Missing),
        "absence is the user's to repair, not a host fault"
    );
}
