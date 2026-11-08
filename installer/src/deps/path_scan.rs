//! PATH lookup for dependency binaries.
//!
//! Scanning is separated from the availability query because it is the only
//! part of `deps` that touches the filesystem. Keeping it behind
//! [`BinaryLocator`] lets the availability logic run against a fixed locator,
//! and keeps every capability handle confined to one module.

use std::path::Path;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Stable, bounded failure labels returned while checking a PATH entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum PathScanFailureCategory {
    /// A PATH entry cannot be represented as UTF-8 for capability-based access.
    NonUtf8Path,
    /// A PATH entry cannot be opened as a directory capability.
    DirectoryOpen,
    /// Candidate metadata cannot be read through the directory capability.
    Metadata,
}

impl PathScanFailureCategory {
    /// Return the low-cardinality field value used by trace events.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonUtf8Path => "non_utf8_path",
            Self::DirectoryOpen => "directory_open",
            Self::Metadata => "metadata",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct PathScanFailure {
    /// Executable being searched for; no filesystem path is retained.
    pub binary_name: String,
    /// Bounded reason a PATH entry was skipped.
    pub category: PathScanFailureCategory,
}

/// Collect the first match and classified failures for one PATH scan.
#[derive(Debug, Default)]
pub(crate) struct PathScanResult {
    pub(crate) binary: Option<std::path::PathBuf>,
    pub(crate) failures: Vec<PathScanFailure>,
}

impl PathScanResult {
    /// Build a result for a found binary and any skipped-entry failures.
    #[cfg(test)]
    pub(crate) const fn new(
        binary: Option<std::path::PathBuf>,
        failures: Vec<PathScanFailure>,
    ) -> Self {
        Self { binary, failures }
    }
}

/// Classify metadata errors, omitting the ordinary miss for an absent candidate.
pub(super) fn metadata_failure_category(
    kind: std::io::ErrorKind,
) -> Option<PathScanFailureCategory> {
    (kind != std::io::ErrorKind::NotFound).then_some(PathScanFailureCategory::Metadata)
}

/// Classify directory-open errors, omitting PATH entries that no longer exist.
pub(super) fn directory_open_failure_category(
    kind: std::io::ErrorKind,
) -> Option<PathScanFailureCategory> {
    (kind != std::io::ErrorKind::NotFound).then_some(PathScanFailureCategory::DirectoryOpen)
}

/// Query PATH and its filesystem entries for one executable.
pub(crate) trait BinaryLocator {
    /// Return the first matching executable and bounded skipped-entry details.
    fn find_binary(&self, binary_name: &str) -> PathScanResult;
}

/// Use the process PATH and host filesystem to locate dependency binaries.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemBinaryLocator;

impl BinaryLocator for SystemBinaryLocator {
    /// Search the process PATH and classify failures from skipped entries.
    fn find_binary(&self, binary_name: &str) -> PathScanResult { find_binary_on_path(binary_name) }
}

/// Check whether a named executable resolves to a file on the current PATH.
///
/// This test helper reports presence only; it does not verify an installed
/// package version.
#[cfg(test)]
fn is_binary_on_path(binary_name: &str) -> bool {
    SystemBinaryLocator
        .find_binary(binary_name)
        .binary
        .is_some()
}

/// Find the first executable match while retaining skipped-entry classifications.
///
/// Return an empty report when PATH is unset. Otherwise, scan entries in their
/// configured order and return the classifications without emitting logs.
fn find_binary_on_path(binary_name: &str) -> PathScanResult {
    let Some(path_var) = std::env::var_os("PATH") else {
        return PathScanResult::default();
    };

    scan_path_directories(std::env::split_paths(&path_var), binary_name)
}

/// Scan ordered PATH entries and retain classified failures for the caller.
///
/// The report preserves every skipped-entry category before the first match so
/// the installer boundary can emit bounded diagnostics without coupling the
/// filesystem query helpers to tracing.
fn scan_path_directories(
    directories: impl IntoIterator<Item = std::path::PathBuf>,
    binary_name: &str,
) -> PathScanResult {
    let mut result = PathScanResult::default();
    for directory in directories {
        result.binary = find_binary_in_directory(&directory, binary_name, &mut result.failures);
        if result.binary.is_some() {
            break;
        }
    }
    result
}

/// Find the first executable candidate relative to one UTF-8 PATH directory.
///
/// Non-UTF-8 entries and directories that cannot be opened are skipped so the
/// caller can continue searching later PATH entries. Candidate metadata is
/// inspected through the opened directory capability rather than ambient
/// filesystem access. Failures are returned as categories without retaining
/// the directory path.
fn find_binary_in_directory(
    directory: &Path,
    binary_name: &str,
    failures: &mut Vec<PathScanFailure>,
) -> Option<std::path::PathBuf> {
    let Some(utf8_directory) = Utf8Path::from_path(directory) else {
        failures.push(PathScanFailure {
            binary_name: binary_name.to_owned(),
            category: PathScanFailureCategory::NonUtf8Path,
        });
        return None;
    };
    let directory_capability = match Dir::open_ambient_dir(utf8_directory, ambient_authority()) {
        Ok(directory_capability) => directory_capability,
        Err(error) => {
            if let Some(category) = directory_open_failure_category(error.kind()) {
                failures.push(PathScanFailure {
                    binary_name: binary_name.to_owned(),
                    category,
                });
            }
            return None;
        }
    };

    let mut failure_categories = Vec::new();
    let candidate = binary_candidates(binary_name).into_iter().find(|name| {
        is_executable_file(
            &directory_capability,
            Utf8Path::new(name),
            &mut failure_categories,
        )
    });
    failures.extend(
        failure_categories
            .into_iter()
            .map(|category| PathScanFailure {
                binary_name: binary_name.to_owned(),
                category,
            }),
    );
    candidate.map(|found| directory.join(found))
}

/// Return executable candidate names in their PATH-search order.
///
/// Unix returns the requested name unchanged. Windows expands extensionless
/// names using `PATHEXT`; an existing extension is preserved as the sole
/// candidate.
fn binary_candidates(binary_name: &str) -> Vec<String> {
    #[cfg(windows)]
    let mut candidates = Vec::new();
    #[cfg(not(windows))]
    let candidates = vec![binary_name.to_owned()];
    #[cfg(windows)]
    {
        if Path::new(binary_name).extension().is_some() {
            candidates.push(binary_name.to_owned());
        } else {
            let lowercase_name = binary_name.to_ascii_lowercase();
            candidates.extend(
                windows_path_extensions()
                    .into_iter()
                    .filter(|extension| !lowercase_name.ends_with(&extension.to_ascii_lowercase()))
                    .map(|extension| format!("{binary_name}{extension}")),
            );
        }
    }
    candidates
}

/// Parse `PATHEXT` into suffixes used to probe Windows PATH entries.
#[cfg(windows)]
fn windows_path_extensions() -> Vec<String> {
    let path_ext = std::env::var_os("PATHEXT")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| std::ffi::OsString::from(".COM;.EXE;.BAT;.CMD"));

    path_ext
        .to_string_lossy()
        .split(';')
        .filter_map(|extension| {
            let trimmed = extension.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.starts_with('.') {
                Some(trimmed.to_owned())
            } else {
                Some(format!(".{trimmed}"))
            }
        })
        .collect()
}

/// Classify a candidate through its containing directory capability.
///
/// Return whether the candidate is a regular file with an executable bit.
///
/// Missing candidates return `false` without a failure category; other
/// metadata errors add the bounded `Metadata` category and also return `false`.
#[cfg(unix)]
fn is_executable_file(
    directory: &Dir,
    candidate: &Utf8Path,
    failures: &mut Vec<PathScanFailureCategory>,
) -> bool {
    use cap_std::fs::PermissionsExt;

    match directory.metadata(candidate) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(error) => {
            if let Some(category) = metadata_failure_category(error.kind()) {
                failures.push(category);
            }
            false
        }
    }
}

/// Classify a candidate as a regular file through its directory capability.
///
/// Missing candidates return `false` without a failure category; other
/// metadata errors add the bounded `Metadata` category. Otherwise, return
/// whether the candidate is a regular file.
#[cfg(not(unix))]
fn is_executable_file(
    directory: &Dir,
    candidate: &Utf8Path,
    failures: &mut Vec<PathScanFailureCategory>,
) -> bool {
    match directory.metadata(candidate) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => {
            if let Some(category) = metadata_failure_category(error.kind()) {
                failures.push(category);
            }
            false
        }
    }
}

#[cfg(test)]
#[path = "path_scan_tests.rs"]
mod path_scan_tests;

#[cfg(test)]
#[path = "path_discovery_tests.rs"]
mod path_discovery_tests;
