//! Turning what a Dylint library path holds into a readiness verdict.
//!
//! The listing is the adapter's read-only seam
//! ([`crate::adapters::library_path`]) and the rules over it live in
//! [`crate::library_path`]. This module maps a listing fault to the port's I/O
//! fault and a classification to the readiness error it deserves, so the
//! prepared value's constructor judges the same evidence the readiness check
//! approved.

use std::ffi::OsStr;

use crate::{
    adapters::library_path::inspect,
    library_path::{SuitePresence, classify},
    ports::{BootstrapFault, CheckPreflightError},
};

/// Confirm that `path` holds exactly one Whitaker suite library.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when no directory on the
/// path holds the suite, [`CheckPreflightError::UserPathAmbiguous`] when more
/// than one does, [`CheckPreflightError::UserPathUnusable`] when Dylint could
/// not search the list at all, and [`BootstrapFault::Io`] when a directory
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
/// Returns [`BootstrapFault::LibrariesMissing`] when the directory holds no
/// suite, [`CheckPreflightError::RecordedPathAmbiguous`] when it holds more
/// than one, [`CheckPreflightError::RecordedPathUnusable`] when the record
/// does not name a searchable directory, and [`BootstrapFault::Io`] when it
/// cannot be read.
pub(super) fn recorded_suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    match presence(path)? {
        SuitePresence::ExactlyOne => Ok(()),
        // An incomplete installation is repairable: reinstalling stages the
        // library the directory lost.
        SuitePresence::Absent => Err(BootstrapFault::LibrariesMissing {
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

/// Inspect `path`, mapping an unreadable entry to the bootstrap's report.
///
/// # Errors
///
/// Returns [`BootstrapFault::Io`] when an entry, or a file inside one, cannot
/// be read for any reason other than absence.
pub(super) fn presence(path: &OsStr) -> Result<SuitePresence, CheckPreflightError> {
    let reports = inspect(path).map_err(|fault| BootstrapFault::Io {
        path: fault.path().to_owned(),
        reason: fault.reason().to_owned(),
    })?;
    Ok(classify(&reports))
}
