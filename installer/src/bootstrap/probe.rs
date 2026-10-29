//! Error-aware filesystem probes for the bootstrap's query paths.
//!
//! `Path::is_file`, `is_dir`, and `exists` answer `false` for every I/O error,
//! so a permission or storage fault would read as "not installed" or "cold"
//! and send the run down the wrong path. These probes treat only `NotFound` as
//! absence and return every other failure. Their scope is the `bootstrap`
//! module and the installer's bootstrap step; other installer code keeps its
//! own error types.

use super::error::BootstrapError;
use camino::Utf8Path;
use std::fs::Metadata;
use std::io::ErrorKind;

/// Return `path`'s metadata, or `None` when nothing exists there.
///
/// # Errors
///
/// Returns [`BootstrapError::Io`] for any failure other than `NotFound`.
pub(crate) fn metadata_if_present(path: &Utf8Path) -> Result<Option<Metadata>, BootstrapError> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(BootstrapError::io(path, &error)),
    }
}

/// Return whether `path` is an existing directory.
///
/// # Errors
///
/// As for `metadata_if_present`.
pub fn is_present_dir(path: &Utf8Path) -> Result<bool, BootstrapError> {
    Ok(metadata_if_present(path)?.is_some_and(|metadata| metadata.is_dir()))
}
