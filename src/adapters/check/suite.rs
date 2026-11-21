//! Turning what a Dylint library path holds into a readiness verdict.
//!
//! The inspection itself lives in [`crate::library_path`], because the
//! prepared value's constructor performs the same check. This module only
//! decides which readiness error a given answer deserves.

use std::ffi::OsStr;

use whitaker_installer::bootstrap::BootstrapError;

use crate::{library_path::SuitePresence, ports::CheckPreflightError};

/// Confirm that `path` holds exactly one Whitaker suite library.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when no directory on the
/// path holds the suite, [`CheckPreflightError::UserPathAmbiguous`] when more
/// than one does, [`CheckPreflightError::UserPathUnusable`] when Dylint could
/// not search the list at all, and a bootstrap I/O error when a directory
/// cannot be read. That last case is a host fault rather than a missing suite:
/// reporting it as "the user's path holds nothing" would send them looking for
/// the wrong problem.
pub(super) fn suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    match presence(path)? {
        SuitePresence::Absent => Err(CheckPreflightError::UserPathHasNoSuite),
        SuitePresence::ExactlyOne => Ok(()),
        SuitePresence::Ambiguous => Err(CheckPreflightError::UserPathAmbiguous),
        SuitePresence::Unusable {
            path: entry,
            reason,
        } => Err(CheckPreflightError::UserPathUnusable {
            path: entry,
            reason,
        }),
    }
}

/// Require a recorded library directory to name exactly one Whitaker suite.
///
/// The installation records one directory per toolchain and target, and Dylint
/// loads whatever that directory holds, so a directory naming two suites is
/// not a directory Dylint can run: it exits `1` without linting. That status
/// is indistinguishable from a lint denial, so the defect is refused here.
///
/// # Errors
///
/// Returns [`BootstrapError::LibrariesMissing`] when the directory holds no
/// suite, [`CheckPreflightError::RecordedPathAmbiguous`] when it holds more
/// than one, [`CheckPreflightError::RecordedPathUnusable`] when the record
/// does not name a searchable directory, and a bootstrap I/O error when it
/// cannot be read.
pub(super) fn recorded_suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    match presence(path)? {
        SuitePresence::ExactlyOne => Ok(()),
        // An incomplete installation is repairable: reinstalling stages the
        // library the directory lost.
        SuitePresence::Absent => Err(BootstrapError::LibrariesMissing {
            path: path.to_string_lossy().into_owned(),
        }
        .into()),
        // A directory holding several suites is a provisioning defect too, but
        // not the same one: a reinstall may or may not replace the stale file,
        // and the repair flow reports "not converged" when it does not.
        SuitePresence::Ambiguous => Err(CheckPreflightError::RecordedPathAmbiguous {
            path: path.to_string_lossy().into_owned(),
        }),
        // The record is Whitaker's own, so the remedy is provisioning even
        // though the entry, not the directory contents, is what is wrong.
        SuitePresence::Unusable {
            path: entry,
            reason,
        } => Err(CheckPreflightError::RecordedPathUnusable {
            path: entry,
            reason,
        }),
    }
}

/// Inspect `path`, mapping an unreadable directory to the bootstrap's report.
fn presence(path: &OsStr) -> Result<SuitePresence, CheckPreflightError> {
    crate::library_path::suite_presence(path).map_err(|fault| {
        BootstrapError::Io {
            path: fault.path().to_owned(),
            reason: fault.reason().to_owned(),
        }
        .into()
    })
}
