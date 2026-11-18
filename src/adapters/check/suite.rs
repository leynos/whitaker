//! Inspecting a Dylint library path for the Whitaker suite.
//!
//! Dylint 6.3.0 refuses to choose between two libraries matching
//! `whitaker_suite` and exits `1`, which is the same status as a lint denial.
//! A path that holds none, or more than one, is therefore reported as a
//! provisioning defect rather than passed to Dylint, where its status would be
//! indistinguishable from the caller's own lint results.

use std::{ffi::OsStr, path::Path};

use whitaker_installer::{
    bootstrap::BootstrapError,
    resolution::SUITE_CRATE,
    scanner::parse_library_filename,
};

use crate::ports::CheckPreflightError;

/// Confirm that `path` holds exactly one Whitaker suite library.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when no directory on the
/// path holds the suite, [`CheckPreflightError::UserPathAmbiguous`] when more
/// than one does, and a bootstrap I/O error when a directory cannot be read.
/// That last case is a host fault rather than a missing suite: reporting it as
/// "the user's path holds nothing" would send them looking for the wrong
/// problem.
pub(super) fn suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    match suite_count(path)? {
        0 => Err(CheckPreflightError::UserPathHasNoSuite),
        1 => Ok(()),
        _ => Err(CheckPreflightError::UserPathAmbiguous),
    }
}

/// Count the Whitaker suite libraries named by a path list.
pub(super) fn suite_count(path: &OsStr) -> Result<usize, CheckPreflightError> {
    std::env::split_paths(path).try_fold(0, |total, directory| Ok(total + suites_in(&directory)?))
}

/// Count the Whitaker suite libraries one directory holds.
///
/// A missing directory is not an error: it is one entry of a path list that an
/// earlier install may have removed.
fn suites_in(directory: &Path) -> Result<usize, CheckPreflightError> {
    let mut entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(io_fault(directory, &error)),
    };
    entries.try_fold(0, |total, found| {
        let entry = found.map_err(|error| io_fault(directory, &error))?;
        Ok(total + usize::from(is_suite_library(&entry)?))
    })
}

/// Report whether a directory entry is a staged Whitaker suite library.
///
/// The entry must be a file, or a symlink to one: a directory named like the
/// suite would satisfy a name-only test while giving Dylint nothing to load.
/// The name is still the authority for which library it is, because the staged
/// file name carries the toolchain the library was built for.
fn is_suite_library(entry: &std::fs::DirEntry) -> Result<bool, CheckPreflightError> {
    if !has_suite_name(&entry.file_name()) {
        return Ok(false);
    }
    match std::fs::metadata(entry.path()) {
        Ok(metadata) => Ok(metadata.is_file()),
        // A symlink whose target has gone still parses as a suite name, but it
        // is not a library Dylint could load.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_fault(&entry.path(), &error)),
    }
}

/// Report whether a file name is a staged Whitaker suite library name.
pub(super) fn has_suite_name(name: &OsStr) -> bool {
    name.to_str()
        .and_then(parse_library_filename)
        .is_some_and(|(krate, _)| krate.as_str() == SUITE_CRATE)
}

/// Turn a directory read failure into the bootstrap's I/O report.
fn io_fault(directory: &Path, error: &std::io::Error) -> CheckPreflightError {
    BootstrapError::Io {
        path: directory.display().to_string(),
        reason: error.to_string(),
    }
    .into()
}
