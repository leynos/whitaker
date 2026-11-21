//! What a Dylint library path list contains.
//!
//! Two places need this answer, and they must agree: the readiness check,
//! which decides whether a user-managed path may reach Dylint at all, and the
//! constructor of the prepared value that Dylint later runs with. Keeping the
//! inspection here means a prepared value cannot name a path the readiness
//! check would have refused.
//!
//! The rules are Dylint 6.3.0's, because Dylint searches the list afterwards.
//! It requires every entry to be an absolute path to an existing directory, it
//! discovers libraries by file name alone, and it refuses to choose between
//! two candidates for `whitaker_suite`. Each of those refusals exits `1`, the
//! status Dylint also uses for a lint denial, so a list that breaks one of
//! them is a provisioning defect rather than something to pass on.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use whitaker_installer::{
    bootstrap::BootstrapError,
    resolution::SUITE_CRATE,
    scanner::parse_library_filename,
};

use crate::ports::CheckPreflightError;

/// An entry of a library path list that is not an absolute path.
pub(crate) const NOT_ABSOLUTE: &str = "is not an absolute path";

/// An entry of a library path list that does not name a directory.
pub(crate) const NOT_A_DIRECTORY: &str = "is not a directory";

/// A suite-named entry that Dylint could not load as a library.
pub(crate) const NOT_A_LIBRARY: &str = "is not a library file";

/// What a Dylint library path list holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SuitePresence {
    /// No directory on the list holds a Whitaker suite library.
    Absent,
    /// Exactly one Whitaker suite library is reachable.
    ExactlyOne,
    /// More than one is reachable, so Dylint would refuse to choose.
    Ambiguous,
    /// The list is not one Dylint can search, so it is refused before Dylint
    /// runs.
    ///
    /// The reason is a predicate over the entry rather than a rendered
    /// message, so the same wording reaches every caller and nothing about a
    /// user's path leaks into a bounded label.
    Unusable {
        /// The entry that cannot be searched.
        path: String,
        /// What is wrong with the entry.
        reason: &'static str,
    },
}

/// A user-managed library path already shown to name exactly one suite.
///
/// The field is private so that holding one is evidence rather than a claim:
/// the only way to obtain the value is [`DirectLibraryPath::new`], which
/// performs the same inspection the readiness check does. Without that, a
/// caller could hand `check` a path Dylint cannot load and receive a status
/// indistinguishable from a lint denial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLibraryPath(OsString);

impl DirectLibraryPath {
    /// Inspect `path` and keep it only if it names exactly one loadable suite.
    ///
    /// # Errors
    ///
    /// Returns [`CheckPreflightError::UserPathHasNoSuite`] when the path names
    /// none, [`CheckPreflightError::UserPathAmbiguous`] when it names more
    /// than one, [`CheckPreflightError::UserPathUnusable`] when Dylint could
    /// not search the list at all, and a bootstrap I/O error when a directory
    /// cannot be read.
    pub fn new(path: OsString) -> Result<Self, CheckPreflightError> {
        match suite_presence(&path) {
            Ok(SuitePresence::ExactlyOne) => Ok(Self(path)),
            Ok(SuitePresence::Absent) => Err(CheckPreflightError::UserPathHasNoSuite),
            Ok(SuitePresence::Ambiguous) => Err(CheckPreflightError::UserPathAmbiguous),
            Ok(SuitePresence::Unusable {
                path: entry,
                reason,
            }) => Err(CheckPreflightError::UserPathUnusable {
                path: entry,
                reason,
            }),
            Err(fault) => Err(BootstrapError::Io {
                path: fault.path().to_owned(),
                reason: fault.reason().to_owned(),
            }
            .into()),
        }
    }

    /// The `DYLINT_LIBRARY_PATH` value to export to the child.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr { &self.0 }
}

/// A directory on the path list that could not be inspected.
///
/// Kept as its own type rather than folded into [`SuitePresence`]: a directory
/// that cannot be read is a host fault, and reporting it as "the path holds
/// nothing" would send the user looking for the wrong problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectFault {
    /// The directory that could not be read.
    path: String,
    /// Why it could not be read.
    reason: String,
}

impl InspectFault {
    /// The directory that could not be read.
    pub(crate) fn path(&self) -> &str { &self.path }

    /// Why it could not be read.
    pub(crate) fn reason(&self) -> &str { &self.reason }
}

/// Judge a Dylint library path list by what Dylint would find in it.
///
/// Entries are validated before anything is read, because Dylint rejects the
/// whole list rather than skipping an entry it cannot search. Discovered
/// library paths are deduplicated, because Dylint stores them in a set: a
/// directory listed twice still holds one library.
pub(crate) fn suite_presence(path: &OsStr) -> Result<SuitePresence, InspectFault> {
    let mut discovered = Vec::new();
    for directory in std::env::split_paths(path) {
        if !directory.is_absolute() {
            return Ok(unusable(&directory, NOT_ABSOLUTE));
        }
        if !directory.is_dir() {
            return Ok(unusable(&directory, NOT_A_DIRECTORY));
        }
        discovered.extend(suites_in(&directory)?);
    }
    discovered.sort();
    discovered.dedup();
    match discovered.as_slice() {
        [] => Ok(SuitePresence::Absent),
        [only] => loadable(only),
        [_, _, ..] => Ok(SuitePresence::Ambiguous),
    }
}

/// List the Whitaker suite libraries one directory holds.
///
/// Discovery is by file name alone, as Dylint's is: whether an entry is a file
/// is decided once the whole list has been counted, because two candidates are
/// ambiguous whatever they are.
fn suites_in(directory: &Path) -> Result<Vec<PathBuf>, InspectFault> {
    let mut discovered = Vec::new();
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        // The entry named a directory a moment ago. If it has since gone there
        // is no suite to find, and a concurrent install removing a staging
        // directory is not a defect in the user's list.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(discovered),
        Err(error) => return Err(fault(directory, &error)),
    };
    for found in entries {
        let entry = found.map_err(|error| fault(directory, &error))?;
        if has_suite_name(&entry.file_name()) {
            discovered.push(entry.path());
        }
    }
    Ok(discovered)
}

/// Judge the single suite library a path list discovered.
///
/// Dylint would try to load it, so it must resolve to a regular file. A
/// directory, or a symlink whose target has gone, carries a suite's name while
/// giving Dylint nothing to load, and the failure it reports uses the status a
/// lint denial uses.
fn loadable(path: &Path) -> Result<SuitePresence, InspectFault> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(SuitePresence::ExactlyOne),
        Ok(_) => Ok(unusable(path, NOT_A_LIBRARY)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(unusable(path, NOT_A_LIBRARY))
        }
        Err(error) => Err(fault(path, &error)),
    }
}

/// Report that a path-list entry is not one Dylint can search.
fn unusable(path: &Path, reason: &'static str) -> SuitePresence {
    SuitePresence::Unusable {
        path: path.display().to_string(),
        reason,
    }
}

/// Report whether a file name is a staged Whitaker suite library name.
pub(crate) fn has_suite_name(name: &OsStr) -> bool {
    name.to_str()
        .and_then(parse_library_filename)
        .is_some_and(|(krate, _)| krate.as_str() == SUITE_CRATE)
}

/// Describe a directory read failure against the directory it concerned.
fn fault(directory: &Path, error: &std::io::Error) -> InspectFault {
    InspectFault {
        path: directory.display().to_string(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
