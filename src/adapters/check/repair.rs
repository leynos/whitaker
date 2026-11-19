//! Lazy provisioning for `whitaker check`.
//!
//! A repair is the one place `check` writes anything, so it keeps its policy
//! together: it installs only for a defect an install can fix, holds the
//! installation lock throughout, and preserves the lint-suite provenance the
//! machine already recorded. Installing the rolling tip over a pinned suite
//! would change which lints run, with no change in the repository being linted.

use std::io::Write;

use whitaker_installer::{
    artefact::suite_ref::SuiteRef,
    bootstrap::{BootstrapError, BootstrapLayout, BootstrapMetadata},
    cli::{InstallArgs, SkipFlags},
    dirs::BaseDirs,
    error::InstallerError,
    orchestration::run_install_for_whitaker_under_lock,
};

use super::preflight_recorded_with;
use crate::{
    cli::check::classify::{PreflightFailure, classify_preflight_failure},
    diagnostics::DiagnosticSettings,
    ports::{CheckPreflightError, CheckRepairError},
};

/// Told once before provisioning starts, so a slow first run explains itself.
const REPAIR_NOTICE: &str = "whitaker: installing missing Whitaker components (runs once)...";

/// Advice for CI, where an implicit install is usually a mistake.
const CI_NOTICE: &str = concat!(
    "whitaker: note: CI is set, so this repair is slower and cannot be reused; ",
    "provision with `whitaker install` and run `whitaker check --no-install` instead",
);

/// Provision the installation while the caller holds the lock.
///
/// The readiness check repeats inside the lock because another process may
/// have provisioned while this one waited; installing again then would repeat
/// work that is already done.
pub(super) fn repair_under_lock(
    dirs: &dyn BaseDirs,
    settings: DiagnosticSettings,
    stderr: &mut dyn Write,
) -> Result<(), CheckRepairError> {
    match preflight_recorded_with(dirs, None) {
        Ok(_) => return Ok(()),
        // Only a defect the installer can fix is worth installing for. A host
        // fault, such as a directory that became unreadable while this process
        // waited for the lock, would turn into "repair failed" and hide the
        // real cause.
        Err(error) if !is_repairable(&error) => {
            return Err(CheckRepairError::Host {
                message: error.to_string(),
            });
        }
        Err(_) => {}
    }
    // Read the pin before installing anything: an installation that recorded
    // one must be rebuilt from that reference, or the repair would silently
    // change which lints run.
    let pin = recorded_suite_pin(dirs)?;
    announce_repair(stderr)?;
    // The installer reserves standard output for machine-readable markers.
    // `check` owns only diagnostics, so the marker is forwarded to the same
    // stream rather than mixed into whatever the user pipes from `check`.
    let mut installer_stdout = std::io::stderr();
    // The lock is already held by the caller, so the install runs under it
    // rather than trying to take it again.
    run_install_for_whitaker_under_lock(
        &repair_arguments(pin.as_ref(), settings),
        &mut installer_stdout,
        stderr,
    )
    .map_err(|error| CheckRepairError::Install {
        message: error.to_string(),
    })
}

/// Report whether provisioning could fix this readiness failure.
const fn is_repairable(error: &CheckPreflightError) -> bool {
    matches!(
        classify_preflight_failure(error),
        PreflightFailure::Repairable(_)
    )
}

/// Explain that a repair is starting.
fn announce_repair(stderr: &mut dyn Write) -> Result<(), CheckRepairError> {
    writeln!(stderr, "{REPAIR_NOTICE}").map_err(|error| host_write(&error))?;
    if std::env::var_os("CI").is_some() {
        writeln!(stderr, "{CI_NOTICE}").map_err(|error| host_write(&error))?;
    }
    Ok(())
}

/// Describe a stream failure that stopped a repair from starting.
fn host_write(error: &std::io::Error) -> CheckRepairError {
    CheckRepairError::Host {
        message: error.to_string(),
    }
}

/// The suite pin the installation recorded, if it recorded one.
///
/// A record that is absent or unusable has no provenance to preserve: the
/// installer is about to write the first usable record for this machine, or
/// replace one that cannot be read at all. Any other read failure is the
/// host's, and installing over it would hide the real fault.
pub(super) fn recorded_suite_pin(
    dirs: &dyn BaseDirs,
) -> Result<Option<SuiteRef>, CheckRepairError> {
    let layout = BootstrapLayout::from_dirs(dirs).map_err(|error| CheckRepairError::Host {
        message: error.to_string(),
    })?;
    match BootstrapMetadata::read(&layout.metadata_path()) {
        Ok(metadata) => metadata.suite_version.map_or(Ok(None), |pin| {
            // A pin that no longer validates is a damaged record. Installing
            // without it would change lint results, so the repair refuses and
            // leaves the choice to an explicit `whitaker install`.
            SuiteRef::try_from(pin.clone())
                .map(Some)
                .map_err(|error| CheckRepairError::Install {
                    message: format!("the recorded lint-suite pin `{pin}` is not usable: {error}"),
                })
        }),
        Err(BootstrapError::NotInstalled { .. } | BootstrapError::Metadata { .. }) => Ok(None),
        Err(error) => Err(CheckRepairError::Host {
            message: error.to_string(),
        }),
    }
}

/// The installer request a lazy repair makes.
///
/// `no_update` is set so that repair fills a gap in the installation rather
/// than silently upgrading a managed clone: an upgrade would change lint
/// results without the user asking. A recorded suite pin is replayed for the
/// same reason, and the root command's `-q`/`-v` flags reach the installer so
/// its progress matches the rest of the invocation.
pub(super) fn repair_arguments(
    pin: Option<&SuiteRef>,
    settings: DiagnosticSettings,
) -> InstallArgs {
    InstallArgs {
        skip: SkipFlags {
            no_update: true,
            ..SkipFlags::default()
        },
        suite_version: pin.cloned(),
        quiet: settings.quiet,
        verbosity: settings.verbosity,
        ..InstallArgs::default()
    }
}

/// Classify a failure to take the installation lock.
pub(super) fn lock_fault(error: &InstallerError) -> CheckRepairError {
    match error {
        InstallerError::InstallLock { .. } => CheckRepairError::Lock {
            message: error.to_string(),
        },
        InstallerError::Bootstrap(BootstrapError::NoDataDirectory) => {
            CheckRepairError::DataDirectory
        }
        _ => CheckRepairError::Host {
            message: error.to_string(),
        },
    }
}
