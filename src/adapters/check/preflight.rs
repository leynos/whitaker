//! Deciding whether the installation can lint, and under which route.
//!
//! Readiness answers one question for two callers: `check` asks it before it
//! runs anything, and the run re-checks the approved path moments later. Both
//! judge the path by [`suite_rule`], so the two steps cannot drift: a path
//! approved by one is approved by the other, and a defect found later carries
//! the same reason an earlier one would have.
//!
//! The route is decided here too, because it is what the approval carries: a
//! recorded installation and a user-managed path are judged by different
//! rules, and re-deriving which one applies at the run would let the answer
//! change between the two questions.

use std::ffi::{OsStr, OsString};

use whitaker_installer::{
    bootstrap::{RunOptions, preflight_with_cargo},
    dirs::{BaseDirs, SystemBaseDirs},
};

use super::{
    approval::Approval,
    fault::fault_of,
    suite::{presence, recorded_suite_present, suite_present},
};
use crate::{
    library_path::SuitePresence,
    ports::{BootstrapFault, CheckPreflightError, DirectLibraryPath, LibraryRoute},
};

/// Inspect `path` and prepare it as the user's own Dylint library path.
///
/// Readiness and this function judge the same listing with the same rules, so
/// a path approved by one is approved by the other. It is public because the
/// value it returns can only be obtained from an inspection: a caller that
/// substitutes the `check` port stages an approved path through the adapter
/// rather than forging one.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when the path names no
/// suite, [`CheckPreflightError::UserPathAmbiguous`] when it names more than
/// one, [`CheckPreflightError::UserPathUnusable`] when Dylint could not search
/// the list, and a bootstrap I/O error when an entry cannot be read.
pub fn prepare_direct_library_path(
    path: OsString,
) -> Result<DirectLibraryPath, CheckPreflightError> {
    // The pure rules judge the adapter's listing; the port's error vocabulary
    // is named here rather than in `src/library_path`, which depends on
    // nothing above it.
    let classified = presence(&path)?;
    refused_unless_exactly_one(classified)?;
    Ok(DirectLibraryPath::from_approved(path))
}

/// Refuse a classification other than exactly one suite.
///
/// # Errors
///
/// Returns [`CheckPreflightError::UserPathHasNoSuite`] when the path names
/// none, [`CheckPreflightError::UserPathAmbiguous`] when it names more than
/// one, and [`CheckPreflightError::UserPathUnusable`] when Dylint could not
/// search the list at all.
fn refused_unless_exactly_one(presence: SuitePresence) -> Result<(), CheckPreflightError> {
    match presence {
        SuitePresence::ExactlyOne => Ok(()),
        SuitePresence::Absent => Err(CheckPreflightError::UserPathHasNoSuite),
        SuitePresence::Ambiguous => Err(CheckPreflightError::UserPathAmbiguous),
        SuitePresence::Unusable { path, reason } => {
            Err(CheckPreflightError::UserPathUnusable { path, reason })
        }
    }
}

/// Ask the installed bootstrap whether it is ready.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// bootstrap.
///
/// # Errors
///
/// Returns [`BootstrapFault::NoDataDirectory`] when the platform data
/// directory cannot be determined, and otherwise whatever
/// [`preflight_recorded_with`] reports for this machine.
pub(super) fn preflight_recorded(
    system_dirs: Option<&SystemBaseDirs>,
    cargo: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(CheckPreflightError::Bootstrap(
            BootstrapFault::NoDataDirectory,
        ));
    };
    preflight_recorded_with(dirs, cargo, None, inherited_path)
}

/// Ask the recorded installation whether it is ready, with an optional
/// library-path override.
///
/// The override is the user's `DYLINT_LIBRARY_PATH` when they set one: the
/// bootstrap still supplies the Clippy snapshot and the driver slot, so only
/// the libraries Dylint loads come from the user. Either way the approved
/// path must name the Whitaker suite: the bootstrap only establishes that a
/// directory exists, and `check` always selects `--lib=whitaker_suite`.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// bootstrap; readiness does not read the environment itself.
///
/// # Errors
///
/// Returns the bootstrap's own readiness defect when the installation is not
/// usable, [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when the override names no single
/// suite Dylint could load, and [`BootstrapFault::LibrariesMissing`],
/// [`CheckPreflightError::RecordedPathAmbiguous`] or
/// [`CheckPreflightError::RecordedPathUnusable`] when the recorded library
/// directory no longer names exactly one suite.
pub fn preflight_recorded_with(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    preflight_recorded_with_cargo(dirs, cargo, library_path, inherited_path)
}

/// [`preflight_recorded_with`] with an explicit `cargo` program.
///
/// The readiness probe runs `cargo dylint --version`, so a test that needs a
/// genuine `PreparedRun` from a host whose installed release may differ
/// supplies its own program here rather than replacing the readiness answer.
///
/// `inherited_path` is this process's `PATH` as the adapter set snapshotted
/// it. It is forwarded so the bootstrap can extend the child's `PATH` on
/// Windows, and is deliberately not read here: a helper that reads the
/// environment cannot be tested with a value the test chose.
///
/// # Errors
///
/// As for [`preflight_recorded_with`].
pub fn preflight_recorded_with_cargo(
    dirs: &dyn BaseDirs,
    cargo: &OsStr,
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let options = RunOptions {
        library_path,
        inherited_path,
    };
    let run = preflight_with_cargo(dirs, cargo, options)
        .map_err(|error| CheckPreflightError::Bootstrap(fault_of(error)))?;
    // The approved path's ownership is decided here, once, and travels with
    // the approval: the run re-check has to ask the same question, and
    // re-deriving the answer from the environment could not.
    let route = library_path.map_or(LibraryRoute::Recorded, |_| LibraryRoute::UserManaged);
    suite_rule(route, run.library_dir())?;
    Ok(Approval::Bootstrap { run, route })
}

/// Apply the suite rule the approved library path's route calls for.
///
/// # Errors
///
/// Returns the same defects as [`recorded_suite_present`] for a recorded
/// directory, and as [`suite_present`] for a user-managed path.
pub fn suite_rule(route: LibraryRoute, path: &OsStr) -> Result<(), CheckPreflightError> {
    match route {
        LibraryRoute::Recorded => recorded_suite_present(path),
        LibraryRoute::UserManaged => suite_present(path),
    }
}

/// Decide what a user-managed library path needs before Dylint can run.
///
/// A recorded installation still provides the snapshot and driver, so the
/// readiness check runs with the user's path as the override. Only when no
/// installation has been recorded does the user's path have to supply
/// everything, and then it is inspected for a Whitaker suite: without one,
/// Dylint would fail with a status that looks like a lint result.
///
/// `inherited_path` is the composition root's `PATH`, passed through to the
/// recorded installation's readiness check.
///
/// # Errors
///
/// Returns the recorded installation's own defect when its readiness check
/// finds one, [`BootstrapFault::NoDataDirectory`] when the platform data
/// directory cannot be determined, [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when no installation is recorded
/// and the path names no single suite Dylint could load, and a bootstrap I/O
/// error when a directory on the path cannot be read.
pub(super) fn preflight_user_managed(
    system_dirs: Option<&SystemBaseDirs>,
    cargo: &OsStr,
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    preflight_user_managed_in(
        system_dirs.map(|dirs| dirs as &dyn BaseDirs),
        cargo,
        path,
        inherited_path,
    )
}

/// Decide the user-managed route with the platform directories as a value.
///
/// The directories are a parameter rather than a lookup so the undeterminable
/// case is a decision this function makes, and a test can drive it. Without a
/// data directory there is no way to tell whether an installation is recorded,
/// so the route reports the host fault both routes share instead of silently
/// running a path the record might contradict.
///
/// `inherited_path` travels with the directories because a recorded
/// installation's readiness check is what consumes it.
///
/// # Errors
///
/// Returns [`BootstrapFault::NoDataDirectory`] when `system_dirs` is absent,
/// the recorded installation's own defect when one is recorded, and
/// [`CheckPreflightError::UserPathHasNoSuite`],
/// [`CheckPreflightError::UserPathAmbiguous`] or
/// [`CheckPreflightError::UserPathUnusable`] when no installation is recorded
/// and `path` names no single suite Dylint could load.
pub(super) fn preflight_user_managed_in(
    system_dirs: Option<&dyn BaseDirs>,
    cargo: &OsStr,
    path: &OsStr,
    inherited_path: Option<&OsStr>,
) -> Result<Approval, CheckPreflightError> {
    let Some(dirs) = system_dirs else {
        return Err(BootstrapFault::NoDataDirectory.into());
    };
    match preflight_recorded_with(dirs, cargo, Some(path), inherited_path) {
        Ok(approval) => Ok(approval),
        // No record means the legacy route: the user's path is the whole
        // installation. Any other defect is the installation's to report.
        Err(CheckPreflightError::Bootstrap(BootstrapFault::NotInstalled { .. })) => {
            // The path is listed once, and the rules judge that one listing,
            // so the prepared value is evidence Dylint may run rather than a
            // second inspection that could disagree with the first.
            Ok(Approval::Direct(prepare_direct_library_path(
                path.to_owned(),
            )?))
        }
        Err(error) => Err(error),
    }
}
