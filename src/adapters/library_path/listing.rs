//! The read-only filesystem seam a Dylint library path is inspected through.
//!
//! [`crate::library_path`] owns the rules but performs no I/O. This module
//! performs the reads those rules need and reports what it found in the shapes
//! the rules consume. Nothing here creates, changes, or removes anything: the
//! listing is a query, so a concurrent install can only make it describe a
//! state that is already out of date, which the rules treat as a defect in the
//! list rather than a reason to write.
//!
//! A path-list entry is classified once, by [`classify_entry`], so the
//! readiness check and the library-path resolution cannot disagree about what
//! an entry is.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::library_path::{EntryListing, EntryReport, InspectFault, ListedFile, has_suite_name};

/// What a path-list entry is, as far as Dylint's search is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    /// The entry names a directory Dylint can search.
    Directory,
    /// The entry names something Dylint cannot search.
    NotADirectory,
    /// The entry names nothing on the host.
    Missing,
}

/// Read every entry of `path` as a Dylint library path list.
///
/// An entry that is not absolute is reported uninspected rather than read,
/// because no filesystem question applies to it. Every other entry is
/// classified once, and a directory's contents are listed with each entry's
/// path and whether it resolves to a regular file.
///
/// Reading stops at the first entry the user must fix. Dylint rejects the whole
/// list at that entry, so a fault in a later entry is not what the user needs to
/// hear about: it would replace a repairable defect in their list with a host
/// error. A fault in an earlier entry is still propagated, because it happened
/// before the list was known to be defective.
///
/// # Errors
///
/// Returns an [`InspectFault`] when an entry's metadata, or a directory's
/// contents, cannot be read for a reason other than absence. Absence is a
/// description of the entry, not a host fault.
pub(crate) fn inspect(path: &OsStr) -> Result<Vec<EntryReport>, InspectFault> {
    // Dylint reads the variable through `std::env::var`, so a list it cannot
    // decode is discarded in full. Refusing it here keeps a value Dylint would
    // ignore from being approved as ready.
    let Some(text) = path.to_str() else {
        return Ok(vec![EntryReport {
            path: PathBuf::from(path),
            listing: EntryListing::NotUnicode,
        }]);
    };
    let mut reports = Vec::new();
    for entry in std::env::split_paths(text) {
        let report = inspect_entry(entry)?;
        let is_defect = matches!(
            report.listing,
            EntryListing::Uninspected | EntryListing::Missing | EntryListing::NotADirectory
        );
        reports.push(report);
        if is_defect {
            break;
        }
    }
    Ok(reports)
}

/// Read one path-list entry.
///
/// # Errors
///
/// As for [`inspect`].
fn inspect_entry(path: PathBuf) -> Result<EntryReport, InspectFault> {
    let listing = if path.is_absolute() {
        match classify_entry(&path)? {
            EntryKind::Directory => list_directory(&path)?,
            EntryKind::NotADirectory => EntryListing::NotADirectory,
            EntryKind::Missing => EntryListing::Missing,
        }
    } else {
        EntryListing::Uninspected
    };
    Ok(EntryReport { path, listing })
}

/// Decide what a path-list entry names, without reading its contents.
///
/// The metadata is inspected explicitly rather than through
/// [`Path::is_dir`], because that method reports every metadata failure as
/// `false`. A directory beneath an inaccessible ancestor would then be
/// reported as "not a directory" — a provisioning defect the user is told to
/// repair — when the truth is that the host refused to look. A missing entry,
/// or one whose metadata succeeds and finds a non-directory, is the user's to
/// fix. Any other failure is a host fault, so it is propagated.
///
/// # Errors
///
/// Returns an [`InspectFault`] when the entry's metadata cannot be read for
/// any reason other than absence.
pub(crate) fn classify_entry(path: &Path) -> Result<EntryKind, InspectFault> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(EntryKind::Directory),
        Ok(_) => Ok(EntryKind::NotADirectory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EntryKind::Missing),
        // A component of the path is a file, so the entry cannot name a
        // directory. That is a property of the list the user supplied, not a
        // host fault, and Dylint reports it the same way.
        Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => {
            Ok(EntryKind::NotADirectory)
        }
        Err(error) => Err(InspectFault::io(path, &error)),
    }
}

/// List the contents of a directory classified as a directory.
///
/// # Errors
///
/// Returns an [`InspectFault`] when the directory or one of its entries cannot
/// be read for a reason other than absence. A directory that has gone since it
/// was classified is reported as missing: a concurrent install removing a
/// staging directory is not a defect in the user's list.
fn list_directory(directory: &Path) -> Result<EntryListing, InspectFault> {
    let found = match std::fs::read_dir(directory) {
        Ok(found) => found,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EntryListing::Missing);
        }
        Err(error) => return Err(InspectFault::io(directory, &error)),
    };
    let mut files = Vec::new();
    for entry in found {
        let read = entry.map_err(|error| InspectFault::io(directory, &error))?;
        files.push(listed_file(&read)?);
    }
    Ok(EntryListing::Directory(files))
}

/// Describe one directory entry, following a symlink to its target.
///
/// Only a suite-named entry is asked for its type. The rules filter by name
/// before reading the flag, and a directory on the path may hold thousands of
/// unrelated files, so asking about all of them would add a `stat` per file to
/// every `check` invocation for answers nothing consumes.
///
/// Following the link matches what Dylint does when it loads the library: a
/// symlink to a regular file is loadable. [`std::fs::DirEntry::metadata`]
/// reports the link itself, so the target is asked for explicitly.
///
/// # Errors
///
/// Returns an [`InspectFault`] when a suite-named entry's metadata cannot be
/// read for a reason other than absence. An entry that has gone, such as a
/// dangling symlink or a file removed mid-listing, is reported as not a
/// regular file.
fn listed_file(entry: &std::fs::DirEntry) -> Result<ListedFile, InspectFault> {
    let path = entry.path();
    let is_suite_candidate = path.file_name().is_some_and(has_suite_name);
    let is_regular_file = match is_suite_candidate.then(|| std::fs::metadata(&path)) {
        Some(Ok(metadata)) => metadata.is_file(),
        Some(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => false,
        Some(Err(error)) => return Err(InspectFault::io(&path, &error)),
        None => false,
    };
    Ok(ListedFile {
        path,
        is_regular_file,
    })
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
