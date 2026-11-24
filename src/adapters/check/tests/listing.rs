//! What the read-only listing seam reports through the readiness check.
//!
//! These cases exercise the seam through `suite_present`: a path list it must
//! refuse before Dylint runs, a symlink it must follow, and a host fault it
//! must report rather than disguise as a defect in the user's list.

use std::ffi::OsStr;

#[cfg(unix)]
use tempfile::TempDir;

use super::super::suite::suite_present;
#[cfg(unix)]
use super::fixtures::{directory_with_suite, path_list, suite_filename};
#[cfg(unix)]
use crate::library_path::NOT_A_LIBRARY;
#[cfg(unix)]
use crate::ports::BootstrapFault;
use crate::{library_path::NOT_ABSOLUTE, ports::CheckPreflightError};

/// Refuse a relative entry, which Dylint rejects as not absolute.
#[test]
fn a_relative_entry_is_refused() {
    let result = suite_present(OsStr::new("lints"));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathUnusable {
            path: "lints".to_owned(),
            reason: NOT_ABSOLUTE,
        }),
        "Dylint rejects a relative entry before searching anything"
    );
}

/// Accept a symlink to a staged suite file.
///
/// The listing resolves the link before deciding whether the entry is a
/// regular file, so the rules see a loadable candidate rather than the link
/// itself. A seam that reported the link's own metadata would refuse the path.
#[cfg(unix)]
#[test]
fn a_symlink_to_a_suite_file_is_accepted() {
    let target = directory_with_suite();
    let linked = TempDir::new().expect("a temporary directory is available");
    std::os::unix::fs::symlink(
        target.path().join(suite_filename()),
        linked.path().join(suite_filename()),
    )
    .expect("the symlink can be created");

    let result = suite_present(&path_list(&[&linked]));

    assert!(
        result.is_ok(),
        "a symlink to a regular suite file is a library Dylint can load, got {result:?}"
    );
}

/// Refuse a dangling symlink named like the suite.
///
/// The listing follows the link, finds nothing, and reports the entry as not a
/// regular file. Dylint would discover the name and fail to load it with the
/// status a lint denial uses, so the defect must be refused first.
#[cfg(unix)]
#[test]
fn a_dangling_symlink_named_like_the_suite_is_unusable() {
    let linked = TempDir::new().expect("a temporary directory is available");
    let suite = linked.path().join(suite_filename());
    std::os::unix::fs::symlink(linked.path().join("removed"), &suite)
        .expect("the dangling symlink can be created");

    let result = suite_present(&path_list(&[&linked]));

    assert_eq!(
        result,
        Err(CheckPreflightError::UserPathUnusable {
            path: suite.display().to_string(),
            reason: NOT_A_LIBRARY,
        }),
        "a suite-named entry with nothing to load is a provisioning defect"
    );
}

/// Report an entry beneath an inaccessible ancestor as a host fault.
///
/// The entry exists and names a directory, but the kernel refuses to resolve
/// it because an ancestor denies traversal. `Path::is_dir` reports that as
/// `false`, which would classify the user's path as a provisioning defect; the
/// metadata failure must reach the caller as a host fault instead. The
/// permission bits are the only portable way to make the refusal happen, and a
/// root-owned process reads through them, so the test skips itself where the
/// host cannot express the failure.
#[cfg(unix)]
#[test]
fn an_entry_beneath_an_inaccessible_ancestor_is_a_host_fault() {
    use std::os::unix::fs::PermissionsExt;

    let outer = TempDir::new().expect("a temporary directory is available");
    let blocked = outer.path().join("blocked");
    let inner = blocked.join("lib");
    std::fs::create_dir_all(&inner).expect("the nested directories can be created");
    let restore = || {
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755))
            .expect("the directory permissions can be restored");
    };
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
        .expect("the directory permissions can be removed");

    if std::fs::metadata(&inner).is_ok() {
        restore();
        return;
    }
    let path =
        std::env::join_paths([inner.as_os_str()]).expect("temporary paths are representable");
    let result = suite_present(&path);
    restore();

    assert!(
        matches!(
            result,
            Err(CheckPreflightError::Bootstrap(BootstrapFault::Io { .. }))
        ),
        "a metadata failure is the host's to report, not the user's to repair: {result:?}"
    );
}
