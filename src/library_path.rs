//! What a Dylint library path list contains.
//!
//! Two places need this answer, and they must agree: the readiness check,
//! which decides whether a user-managed path may reach Dylint at all, and the
//! constructor of the prepared value that Dylint later runs with. Keeping the
//! inspection here means a prepared value cannot name a path the readiness
//! check would have refused.
//!
//! Dylint 6.3.0 refuses to choose between two libraries matching
//! `whitaker_suite` and exits `1`, which is the same status as a lint denial.
//! A path holding none, or more than one, is therefore a provisioning defect
//! rather than something to pass to Dylint.

use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use whitaker_installer::{
    bootstrap::BootstrapError,
    resolution::SUITE_CRATE,
    scanner::parse_library_filename,
};

use crate::ports::CheckPreflightError;

/// What a Dylint library path list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuitePresence {
    /// No directory on the list holds a Whitaker suite library.
    Absent,
    /// Exactly one Whitaker suite library is reachable.
    ExactlyOne,
    /// More than one is reachable, so Dylint would refuse to choose.
    Ambiguous,
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
    /// Inspect `path` and keep it only if it names exactly one suite.
    ///
    /// # Errors
    ///
    /// Returns [`CheckPreflightError::UserPathHasNoSuite`] when the path names
    /// none, [`CheckPreflightError::UserPathAmbiguous`] when it names more
    /// than one, and a bootstrap I/O error when a directory cannot be read.
    pub fn new(path: OsString) -> Result<Self, CheckPreflightError> {
        match suite_presence(&path) {
            Ok(SuitePresence::ExactlyOne) => Ok(Self(path)),
            Ok(SuitePresence::Absent) => Err(CheckPreflightError::UserPathHasNoSuite),
            Ok(SuitePresence::Ambiguous) => Err(CheckPreflightError::UserPathAmbiguous),
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

/// Count the Whitaker suite libraries named by a path list.
pub(crate) fn suite_presence(path: &OsStr) -> Result<SuitePresence, InspectFault> {
    let count = std::env::split_paths(path)
        .try_fold(0, |total, directory| Ok(total + suites_in(&directory)?))?;
    Ok(match count {
        0 => SuitePresence::Absent,
        1 => SuitePresence::ExactlyOne,
        _ => SuitePresence::Ambiguous,
    })
}

/// Count the Whitaker suite libraries one directory holds.
///
/// A missing directory is not an error: it is one entry of a path list that an
/// earlier install may have removed.
fn suites_in(directory: &Path) -> Result<usize, InspectFault> {
    let mut entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(fault(directory, &error)),
    };
    entries.try_fold(0, |total, found| {
        let entry = found.map_err(|error| fault(directory, &error))?;
        Ok(total + usize::from(is_suite_library(&entry)?))
    })
}

/// Report whether a directory entry is a staged Whitaker suite library.
///
/// The entry must be a file, or a symlink to one: a directory named like the
/// suite would satisfy a name-only test while giving Dylint nothing to load.
/// The name is still the authority for which library it is, because the staged
/// file name carries the toolchain the library was built for.
fn is_suite_library(entry: &std::fs::DirEntry) -> Result<bool, InspectFault> {
    if !has_suite_name(&entry.file_name()) {
        return Ok(false);
    }
    match std::fs::metadata(entry.path()) {
        Ok(metadata) => Ok(metadata.is_file()),
        // A symlink whose target has gone still parses as a suite name, but it
        // is not a library Dylint could load.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(fault(&entry.path(), &error)),
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
mod tests {
    //! The prepared value's promise: it names only a path Dylint can load.

    use std::ffi::OsString;

    use camino::Utf8PathBuf;
    use rstest::rstest;
    use tempfile::TempDir;
    use whitaker_installer::{crate_name::CrateName, stager::Stager};

    use super::DirectLibraryPath;
    use crate::ports::CheckPreflightError;

    /// A directory holding a suite library for each named toolchain.
    fn holding(toolchains: &[&str]) -> (TempDir, OsString) {
        let directory = TempDir::new().expect("a temporary directory is available");
        for toolchain in toolchains {
            let filename = Stager::new(Utf8PathBuf::from("unused"), toolchain)
                .staged_filename(&CrateName::from(super::SUITE_CRATE));
            std::fs::write(directory.path().join(filename), b"")
                .expect("the suite file can be created");
        }
        let path = directory.path().as_os_str().to_owned();
        (directory, path)
    }

    #[test]
    fn a_path_naming_the_suite_becomes_a_prepared_value() {
        let (_directory, path) = holding(&["nightly-2026-05-28"]);

        let prepared =
            DirectLibraryPath::new(path.clone()).expect("one staged suite makes the path loadable");

        assert_eq!(
            prepared.as_os_str(),
            path,
            "the prepared value must keep the path it validated"
        );
    }

    #[rstest]
    #[case::no_suite(&[])]
    #[case::two_suites(&["nightly-2026-05-28", "nightly-2026-05-29"])]
    fn a_path_dylint_could_not_load_is_refused(#[case] toolchains: &[&str]) {
        let (_directory, path) = holding(toolchains);

        let refused = DirectLibraryPath::new(path);

        assert!(
            matches!(
                refused,
                Err(CheckPreflightError::UserPathHasNoSuite
                    | CheckPreflightError::UserPathAmbiguous)
            ),
            "a path Dylint could not load must not become a prepared value: {refused:?}"
        );
    }
}
