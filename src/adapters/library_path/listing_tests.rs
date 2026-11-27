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

/// Describe an entry that is not a directory without calling it a host fault.
///
/// An entry that names nothing and an entry that names a regular file are both
/// descriptions of the list, not failures to read it, and the seam must keep
/// them apart so the rules can refuse each as the user's to fix.
#[rstest]
#[case::missing("removed-by-a-later-uninstall", false, EntryListing::Missing)]
#[case::file("not-a-directory", true, EntryListing::NotADirectory)]
fn an_entry_that_is_not_a_directory_is_described(
    #[case] name: &str,
    #[case] is_file: bool,
    #[case] expected: EntryListing,
) {
    let outer = TempDir::new().expect("a temporary directory is available");
    let entry = outer.path().join(name);
    if is_file {
        std::fs::write(&entry, b"").expect("the file can be created");
    }
    let path = std::env::join_paths([entry.as_os_str()]).expect("paths are representable");

    let reports = inspect(&path).expect("a description of the list is not a host fault");

    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&expected),
        "the seam must describe what the entry names"
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

/// Report an entry beneath a file as unusable, never as a host fault.
///
/// A component of the path is a regular file, so the entry cannot name a
/// directory. That is a defect in the list the user supplied, and both
/// classifications the platforms produce for it are refusals of the entry:
/// Unix reports `ENOTDIR`, which is classified as [`EntryKind::NotADirectory`],
/// and Windows reports absence, which is [`EntryKind::Missing`]. Either way the
/// host answered the question, so neither may become an [`InspectFault`].
#[test]
fn classify_entry_reports_a_file_component_as_unusable() {
    let outer = TempDir::new().expect("a temporary directory is available");
    let file = outer.path().join("a-file");
    std::fs::write(&file, b"").expect("the file can be created");
    let beneath = file.join("lib");

    let classified = classify_entry(&beneath);

    assert!(
        matches!(
            classified,
            Ok(EntryKind::NotADirectory | EntryKind::Missing)
        ),
        "a path through a file is the user's to fix, not a host fault: {classified:?}"
    );
}

/// Stop listing at the first entry the user must fix.
///
/// Dylint rejects the whole list at that entry, so a fault in a later entry is
/// not what the user needs to hear: reporting it would replace a repairable
/// defect in their list with a host error.
#[cfg(unix)]
#[test]
fn listing_stops_at_the_first_defect() {
    use std::os::unix::fs::PermissionsExt;

    let outer = TempDir::new().expect("a temporary directory is available");
    let blocked = outer.path().join("blocked");
    std::fs::create_dir(&blocked).expect("the directory can be created");
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
        .expect("the directory permissions can be removed");
    let unreadable = blocked.join("lib");
    if std::fs::metadata(&unreadable).is_ok() {
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755))
            .expect("the directory permissions can be restored");
        return;
    }
    let missing = outer.path().join("gone");
    let path = std::env::join_paths([missing.as_os_str(), unreadable.as_os_str()])
        .expect("paths are representable");

    let listing = inspect(&path);

    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755))
        .expect("the directory permissions can be restored");
    let reports = listing.expect("a defect before the fault keeps it out of the answer");
    assert_eq!(
        reports.len(),
        1,
        "listing must stop at the entry the user has to fix"
    );
    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&EntryListing::Missing),
        "the defect, not the later fault, is what the user is told about"
    );
}

/// Refuse a library path list Dylint could not decode.
///
/// Dylint reads `DYLINT_LIBRARY_PATH` through `std::env::var`, so a value that
/// is not Unicode is discarded in full. Approving it here would let Dylint's
/// "no library" failure pass through as a lint result.
#[cfg(unix)]
#[test]
fn a_non_unicode_path_list_is_refused() {
    use std::os::unix::ffi::OsStringExt;

    let path = OsString::from_vec(b"/lints/\xff".to_vec());

    let reports = inspect(&path).expect("an undecodable list is not itself a fault");

    assert_eq!(
        reports.first().map(|report| &report.listing),
        Some(&EntryListing::NotUnicode),
        "a list Dylint cannot decode must be reported as such"
    );
}
