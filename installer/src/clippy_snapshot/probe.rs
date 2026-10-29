//! Error-aware filesystem probes for snapshot queries.
//!
//! `Path::exists`, `is_dir`, and `is_file` answer `false` for every I/O error,
//! so a permission or storage fault would read as "not installed" or "bundle
//! missing" and steer provisioning or verification down the wrong path. These
//! probes treat only `NotFound` as absence and return every other failure as
//! [`ClippySnapshotError::Io`]. They mirror `bootstrap::probe` for this
//! module's own error type and are used only within `clippy_snapshot`.

use super::error::ClippySnapshotError;
use camino::Utf8Path;
use std::fs::Metadata;
use std::io::ErrorKind;

/// Return `path`'s metadata, or `None` when nothing exists there.
fn metadata_if_present(path: &Utf8Path) -> Result<Option<Metadata>, ClippySnapshotError> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ClippySnapshotError::io(path, &error)),
    }
}

/// Return whether anything exists at `path`.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
pub(crate) fn is_present(path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
    Ok(metadata_if_present(path)?.is_some())
}

/// Return whether `path` is an existing directory.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
pub(crate) fn is_present_dir(path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
    Ok(metadata_if_present(path)?.is_some_and(|metadata| metadata.is_dir()))
}

/// Return whether `path` is an existing regular file.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
pub(crate) fn is_present_file(path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
    Ok(metadata_if_present(path)?.is_some_and(|metadata| metadata.is_file()))
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
