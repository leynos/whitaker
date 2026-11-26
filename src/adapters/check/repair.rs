//! Lazy provisioning for `whitaker check`.
//!
//! A repair is the one place `check` writes anything, so it keeps its policy
//! together: it installs only for a defect an install can fix, holds the
//! installation lock throughout, and preserves the lint-suite provenance the
//! machine already recorded. Installing the rolling tip over a pinned suite
//! would change which lints run, with no change in the repository being linted.

use std::{ffi::OsStr, io::Write};

use whitaker_installer::{
    artefact::suite_ref::SuiteRef,
    bootstrap::{BootstrapError, BootstrapLayout, BootstrapMetadata},
    cli::{InstallArgs, SkipFlags},
    dirs::BaseDirs,
    error::InstallerError,
    orchestration::run_install_for_whitaker_under_lock,
};

use super::preflight_recorded_with;
use crate::ports::{
    CheckPreflightError,
    CheckRepairError,
    PreflightFailure,
    RepairVerbosity,
    classify_preflight_failure,
};

/// Told once before provisioning starts, so a slow first run explains itself.
pub(super) const REPAIR_NOTICE: &str =
    "whitaker: installing missing Whitaker components (runs once)...";

/// Advice for CI, where an implicit install is usually a mistake.
pub(super) const CI_NOTICE: &str = concat!(
    "whitaker: note: CI is set, so this repair is slower and cannot be reused; ",
    "provision with `whitaker install` and run `whitaker check --no-install` instead",
);

/// What the readiness check repeated under the lock found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Readiness {
    /// The installation became ready while this process waited for the lock.
    Ready,
    /// An install can mend it.
    Repairable,
    /// The host is at fault, so installing would hide the real cause.
    Host {
        /// The failure as the user should read it.
        message: String,
    },
}

/// What a locked repair does, apart from where it writes.
#[derive(Debug, Clone, Copy)]
pub(super) struct RepairRequest {
    /// The root flags the repair inherits.
    pub verbosity: RepairVerbosity,
    /// Whether the process runs in CI, where an implicit install is a mistake.
    pub is_ci: bool,
}

/// The two streams a locked repair writes to.
pub(super) struct RepairStreams<'a> {
    /// Where the installer's machine-readable markers go.
    ///
    /// The installer reserves standard output for those markers, and `check`
    /// owns only diagnostics, so the production caller points this at the
    /// same stream as `diagnostics`. Keeping the two apart lets the stream a
    /// marker reached be observed.
    pub installer: &'a mut dyn Write,
    /// Where the repair's own notices and the installer's progress go.
    pub diagnostics: &'a mut dyn Write,
}

/// The host facts a locked repair reads readiness with.
///
/// `inherited_path` is this process's own `PATH`, read once at the
/// composition root and handed down as a value. Readiness passes it to the
/// bootstrap, which needs it only to extend the child's `PATH` with the
/// pinned toolchain on Windows; nothing below the composition root reads the
/// environment itself.
#[derive(Clone, Copy)]
pub(super) struct HostEnv<'a> {
    /// The platform directories the installation lives under.
    pub(super) dirs: &'a dyn BaseDirs,
    /// The caller's `PATH`, or `None` when the host did not set one.
    pub(super) inherited_path: Option<&'a OsStr>,
    /// The program the readiness probe runs.
    pub(super) cargo: &'a OsStr,
}

/// The two steps a locked repair runs, as a seam a test can replace.
///
/// Readiness repeats inside the lock because another process may have
/// provisioned while this one waited; installing again then would repeat work
/// that is already done. Keeping the re-check and the install behind one trait
/// lets a test drive every decision without running a real installation.
pub(super) trait RepairSteps {
    /// Answer whether the installation is ready now the lock is held.
    fn readiness(&self, host: HostEnv<'_>) -> Readiness;

    /// Install Whitaker under the lock the caller holds.
    ///
    /// `stdout` receives the installer's machine-readable markers and
    /// `stderr` its progress and errors.
    ///
    /// # Errors
    ///
    /// Returns the installer's own error when the operation cannot complete.
    fn install(
        &mut self,
        arguments: &InstallArgs,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<(), InstallerError>;
}

/// The steps a real repair runs.
pub(super) struct SystemSteps;

impl RepairSteps for SystemSteps {
    fn readiness(&self, host: HostEnv<'_>) -> Readiness {
        match preflight_recorded_with(host.dirs, host.cargo, None, host.inherited_path) {
            Ok(_) => Readiness::Ready,
            // Only a defect the installer can fix is worth installing for. A
            // host fault, such as a directory that became unreadable while
            // this process waited for the lock, would turn into "repair
            // failed" and hide the real cause.
            Err(error) if !is_repairable(&error) => Readiness::Host {
                message: error.to_string(),
            },
            Err(_) => Readiness::Repairable,
        }
    }

    fn install(
        &mut self,
        arguments: &InstallArgs,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<(), InstallerError> {
        // The lock is already held by the caller, so the install runs under it
        // rather than trying to take it again.
        run_install_for_whitaker_under_lock(arguments, stdout, stderr)
    }
}

/// Build the request a lazy repair makes from the environment's CI answer.
///
/// The lookup itself stays in the adapter's composition root; this function is
/// the decision, so a test can pin both branches without mutating the process
/// environment. The answer is borrowed because only its presence matters: the
/// value a platform sets for `CI` is otherwise unused.
pub(super) const fn repair_request(
    verbosity: RepairVerbosity,
    ci: Option<&OsStr>,
) -> RepairRequest {
    RepairRequest {
        verbosity,
        is_ci: ci.is_some(),
    }
}

/// The stream a production repair hands the installer's machine markers.
///
/// `check` owns only diagnostics, and its caller reads standard error, so the
/// markers take a stream of their own rather than sharing the diagnostics the
/// command reports through. Naming the choice here lets the stream test assert
/// the same selection production makes instead of restating it.
#[must_use]
pub(super) fn marker_stream() -> std::io::Stderr { std::io::stderr() }

/// Provision the installation while the caller holds the lock.
///
/// The installer reserves standard output for its machine-readable markers,
/// and `check` owns only diagnostics, so the markers reach standard error
/// rather than whatever the user pipes from `check`. The stream choice lives
/// beside the real steps so a test can drive this function with a fake
/// installer and observe which stream a marker actually reached.
///
/// # Errors
///
/// Returns [`CheckRepairError::Host`] when the host is at fault,
/// [`CheckRepairError::Output`] when a notice cannot be written,
/// [`CheckRepairError::Install`] when the installer fails, and whatever
/// [`recorded_suite_pin`] reports for an unusable record.
pub(super) fn repair_production(
    host: HostEnv<'_>,
    request: RepairRequest,
    streams: RepairStreams<'_>,
    steps: &mut dyn RepairSteps,
) -> Result<(), CheckRepairError> {
    repair_with(host, request, streams, steps)
}

/// Run a locked repair through `steps`.
pub(super) fn repair_with(
    host: HostEnv<'_>,
    request: RepairRequest,
    streams: RepairStreams<'_>,
    steps: &mut dyn RepairSteps,
) -> Result<(), CheckRepairError> {
    match steps.readiness(host) {
        Readiness::Ready => return Ok(()),
        Readiness::Host { message } => return Err(CheckRepairError::Host { message }),
        Readiness::Repairable => {}
    }
    // Read the pin before installing anything: an installation that recorded
    // one must be rebuilt from that reference, or the repair would silently
    // change which lints run.
    let pin = recorded_suite_pin(host.dirs)?;
    let RepairStreams {
        installer,
        diagnostics,
    } = streams;
    announce_repair(diagnostics, request.is_ci)?;
    steps
        .install(
            &repair_arguments(pin.as_ref(), request.verbosity),
            installer,
            diagnostics,
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
///
/// The notice is written before anything is decided, so a stream that refuses
/// it has stopped Whitaker writing its own output: that is the output failure
/// ADR 0017 names, not a host fault. Reporting it as a host fault would tell
/// the user their filesystem failed when the stream may simply have closed.
///
/// # Errors
///
/// Returns [`CheckRepairError::Output`] when the notice cannot be written.
fn announce_repair(stderr: &mut dyn Write, is_ci: bool) -> Result<(), CheckRepairError> {
    writeln!(stderr, "{REPAIR_NOTICE}").map_err(output_write)?;
    if is_ci {
        writeln!(stderr, "{CI_NOTICE}").map_err(output_write)?;
    }
    Ok(())
}

/// Describe a stream failure that stopped a repair from starting.
const fn output_write(error: std::io::Error) -> CheckRepairError {
    CheckRepairError::Output { source: error }
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
        // Nothing installed means nothing to preserve, so an unpinned repair
        // is the same suite the machine had (none).
        Err(BootstrapError::NotInstalled { .. }) => Ok(None),
        // A record that exists but cannot be read may have carried a pin, and
        // this repair cannot know which suite it replaced. Installing the
        // rolling tip would silently change which lints run, so the choice
        // goes back to the user as an explicit `whitaker install`.
        Err(BootstrapError::Metadata { .. }) => Err(CheckRepairError::Install {
            message: "the installed lint-suite pin cannot be read, so an automatic repair would \
                      change which lints run; run `whitaker install` to choose the suite \
                      explicitly"
                .to_owned(),
        }),
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
pub(super) fn repair_arguments(pin: Option<&SuiteRef>, verbosity: RepairVerbosity) -> InstallArgs {
    InstallArgs {
        skip: SkipFlags {
            no_update: true,
            ..SkipFlags::default()
        },
        suite_version: pin.cloned(),
        quiet: verbosity.quiet,
        verbosity: verbosity.verbosity,
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
