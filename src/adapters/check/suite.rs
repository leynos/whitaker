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
/// than one does, and a bootstrap I/O error when a directory cannot be read.
/// That last case is a host fault rather than a missing suite: reporting it as
/// "the user's path holds nothing" would send them looking for the wrong
/// problem.
pub(super) fn suite_present(path: &OsStr) -> Result<(), CheckPreflightError> {
    match presence(path)? {
        SuitePresence::Absent => Err(CheckPreflightError::UserPathHasNoSuite),
        SuitePresence::ExactlyOne => Ok(()),
        SuitePresence::Ambiguous => Err(CheckPreflightError::UserPathAmbiguous),
    }
}

/// Report whether `path` holds at least one Whitaker suite library.
///
/// A recorded installation may hold libraries for several toolchains, so the
/// recorded path is asked only whether the suite is there at all.
pub(super) fn suite_available(path: &OsStr) -> Result<bool, CheckPreflightError> {
    Ok(!matches!(presence(path)?, SuitePresence::Absent))
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
