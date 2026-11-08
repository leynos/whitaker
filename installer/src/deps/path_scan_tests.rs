//! Verify that PATH scan failure categories remain stable and bounded.

use super::{PathScanFailureCategory, directory_open_failure_category, metadata_failure_category};

/// Treat an absent PATH candidate as routine while preserving other failures.
#[test]
fn metadata_failures_exclude_missing_candidates() {
    assert_eq!(
        metadata_failure_category(std::io::ErrorKind::NotFound),
        None
    );
    assert_eq!(
        metadata_failure_category(std::io::ErrorKind::PermissionDenied),
        Some(PathScanFailureCategory::Metadata)
    );
}

/// Treat disappeared PATH directories as misses while preserving other errors.
#[test]
fn directory_open_failures_exclude_missing_directories() {
    assert_eq!(
        directory_open_failure_category(std::io::ErrorKind::NotFound),
        None
    );
    assert_eq!(
        directory_open_failure_category(std::io::ErrorKind::PermissionDenied),
        Some(PathScanFailureCategory::DirectoryOpen)
    );
}

/// Keep path-scan failure category labels stable and bounded.
#[test]
fn path_scan_failure_categories_are_stable_and_bounded() {
    assert_eq!(
        PathScanFailureCategory::NonUtf8Path.as_str(),
        "non_utf8_path"
    );
    assert_eq!(
        PathScanFailureCategory::DirectoryOpen.as_str(),
        "directory_open"
    );
    assert_eq!(PathScanFailureCategory::Metadata.as_str(), "metadata");
}
