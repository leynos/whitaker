//! Production adapters for installer, inventory, and Dylint subprocess ports.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    process::{Command, ExitStatus},
    time::{Duration, Instant},
};

use whitaker_installer::{
    bootstrap::is_installed,
    dirs::{BaseDirs, SystemBaseDirs},
    list::{collect_installed, detect_active_toolchain},
    orchestration::{run_bootstrap_command_with_library_path, run_install_for_whitaker},
};

use self::{
    library_path::active_library_path,
    telemetry::{ForwardOutcome, ForwardSource},
};
use crate::ports::{
    CliError,
    InstallRequest,
    InstallService,
    LintInventory,
    LintRunner,
    ListRequest,
    Services,
    StagedInventory,
};

pub mod library_path;
pub mod telemetry;

/// The system-backed adapters used by the `whitaker` executable.
#[derive(Debug, Default)]
pub struct SystemServices;

impl SystemServices {
    /// Creates the production adapter set.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = ports;
    /// ```
    #[must_use]
    pub const fn new() -> Self { Self }

    /// Borrows the implementation through the command ports.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = (ports.install, ports.inventory, ports.lint_runner);
    /// ```
    #[must_use]
    pub fn ports(&self) -> Services<'_> {
        Services {
            install: self,
            inventory: self,
            lint_runner: self,
        }
    }
}

impl InstallService for SystemServices {
    /// Run a root `whitaker install` and translate installer failures.
    ///
    /// The installer's error type belongs to the installer crate, so it is
    /// flattened into a message at this boundary rather than re-exported.
    fn install(
        &self,
        request: &InstallRequest,
        stdout: &mut dyn std::io::Write,
        stderr: &mut dyn std::io::Write,
    ) -> Result<(), CliError> {
        run_install_for_whitaker(&request.arguments, stdout, stderr).map_err(|error| {
            CliError::Install {
                message: error.to_string(),
            }
        })
    }
}

impl LintInventory for SystemServices {
    /// Collect the installed-lint inventory for `whitaker ls`.
    ///
    /// The active toolchain is display metadata only, so an unreadable
    /// `rust-toolchain.toml` degrades the listing instead of failing it. The
    /// library-path resolution that acts on the toolchain uses the typed
    /// detection result instead.
    fn list(&self, request: &ListRequest) -> Result<StagedInventory, CliError> {
        let installed =
            collect_installed(&request.arguments).map_err(|error| CliError::Inventory {
                message: error.to_string(),
            })?;
        Ok(StagedInventory {
            installed,
            active_toolchain: detect_active_toolchain(),
        })
    }
}

impl LintRunner for SystemServices {
    /// Run the forwarded invocation through the installed bootstrap.
    ///
    /// Running it here rather than as a bare `cargo dylint` is what makes a
    /// cold run work: the bootstrap exports the verified Clippy snapshot and
    /// the managed driver slot, and serializes the first driver build. The
    /// library path is the one this crate resolved, so Dylint can still pick
    /// the toolchain a project declares, and an explicit
    /// `DYLINT_LIBRARY_PATH` stays authoritative.
    ///
    /// A machine with no recorded bootstrap falls back to starting
    /// `cargo dylint` directly. That keeps the legacy forwarding contract for
    /// a source checkout or a manually configured `DYLINT_LIBRARY_PATH`, and
    /// there is no installed bootstrap for that path to bypass.
    fn forward(&self, args: &[OsString], stderr: &mut dyn Write) -> Result<i32, CliError> {
        // Resolve the platform directories once so every lookup in this
        // invocation sees the same view of the installation.
        let system_dirs = SystemBaseDirs::new();
        let dirs = system_dirs.as_ref().map(|system| system as &dyn BaseDirs);
        let source = forward_source(dirs);
        let started = Instant::now();
        let library_path = match active_library_path(dirs) {
            Ok(path) => path,
            Err(error) => return Err(report_resolution_failure(source, started, error)),
        };
        telemetry::record_library_path(library_path.kind(), library_path.candidate_count());
        let forwarding = Forwarding {
            args,
            stderr,
            source,
            library_path: library_path.as_deref(),
            started,
        };
        telemetry::forward_span(source, library_path.kind())
            .in_scope(|| dispatch_forward(forwarding))
    }
}

/// Report whether an installation recorded a bootstrap to run from.
fn has_installed_bootstrap(dirs: Option<&dyn BaseDirs>) -> bool { dirs.is_some_and(is_installed) }

/// Choose the forwarding route this machine supports.
///
/// The route is a property of the installation, not of the arguments, so it is
/// decided before the library path is resolved and can label a resolution
/// failure as well as a completed run.
fn forward_source(dirs: Option<&dyn BaseDirs>) -> ForwardSource {
    if has_installed_bootstrap(dirs) {
        ForwardSource::Bootstrap
    } else {
        ForwardSource::Direct
    }
}

/// One forwarded invocation, as the adapter sees it.
///
/// The fields travel together through dispatch and reporting, so they are
/// grouped rather than threaded through every helper as separate parameters.
struct Forwarding<'a> {
    /// The arguments to hand to Dylint.
    args: &'a [OsString],
    /// Where the child's own diagnostics are written.
    stderr: &'a mut dyn Write,
    /// The route this machine supports.
    source: ForwardSource,
    /// The resolved library path, if any.
    library_path: Option<&'a OsStr>,
    /// When the command started, so resolution and dispatch share one clock.
    started: Instant,
}

/// Run the forwarded invocation and record how it ended.
///
/// The elapsed time covers resolution and dispatch together, because a user
/// waiting on `whitaker` experiences both as one command.
fn dispatch_forward(forwarding: Forwarding<'_>) -> Result<i32, CliError> {
    let Forwarding {
        args,
        stderr,
        source,
        library_path,
        started,
    } = forwarding;
    let result = match source {
        ForwardSource::Bootstrap => Ok(run_bootstrap_command_with_library_path(
            args,
            library_path,
            stderr,
        )),
        ForwardSource::Direct => forward_without_bootstrap(args, library_path),
    };
    let elapsed = started.elapsed();
    result.map_or_else(
        |error| Err(record_dispatch_failure(source, error, elapsed)),
        |code| Ok(record_completion(source, code, elapsed)),
    )
}

/// Record a finished child and return the code this process should exit with.
fn record_completion(source: ForwardSource, code: i32, elapsed: Duration) -> i32 {
    let outcome = ForwardOutcome::classify_exit(code);
    telemetry::record_forward(source, outcome, elapsed);
    tracing::debug!(
        operation = "forward",
        source = source.label(),
        outcome = outcome.label(),
        exit_code = code,
        elapsed_seconds = elapsed.as_secs_f64(),
        "forwarded invocation finished"
    );
    code
}

/// Record a child that could not be started and hand the error back.
fn record_dispatch_failure(source: ForwardSource, error: CliError, elapsed: Duration) -> CliError {
    telemetry::record_forward(source, ForwardOutcome::DispatchFailed, elapsed);
    tracing::error!(
        operation = "forward",
        source = source.label(),
        outcome = ForwardOutcome::DispatchFailed.label(),
        error_category = telemetry::error_category(&error),
        elapsed_seconds = elapsed.as_secs_f64(),
        "forwarded invocation could not be dispatched"
    );
    error
}

/// Record a library-path failure and hand the error back to the caller.
///
/// Returning the error keeps the reporting at the boundary that owns the
/// outcome, so the command layer still decides the process exit code.
fn report_resolution_failure(source: ForwardSource, started: Instant, error: CliError) -> CliError {
    let elapsed = started.elapsed();
    telemetry::record_forward(source, ForwardOutcome::LibraryPathFailed, elapsed);
    tracing::error!(
        operation = "forward",
        source = source.label(),
        outcome = ForwardOutcome::LibraryPathFailed.label(),
        error_category = telemetry::error_category(&error),
        elapsed_seconds = elapsed.as_secs_f64(),
        "the Dylint library path could not be resolved"
    );
    error
}

/// Run `cargo dylint` directly, for a machine with no installed bootstrap.
fn forward_without_bootstrap(
    args: &[OsString],
    library_path: Option<&OsStr>,
) -> Result<i32, CliError> {
    let mut command = Command::new("cargo");
    command.arg("dylint").args(args);
    if let Some(path) = library_path {
        command.env("DYLINT_LIBRARY_PATH", path);
    }
    let status = command
        .status()
        .map_err(|source| CliError::Forward { source })?;
    Ok(child_exit_code(status))
}

/// Map a child's status to the code this process should exit with.
fn child_exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| signal_exit_code(status))
        .unwrap_or(1)
}

/// Map a terminating signal to the conventional 128 + signal exit code.
///
/// A child killed by a signal has no ordinary exit code, but this process
/// still has to report something; 128 + signal is the shell's convention.
#[cfg(unix)]
fn signal_exit_code(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal().map(|signal| 128 + signal)
}

/// Report no signal-derived exit code on platforms without POSIX signals.
#[cfg(not(unix))]
const fn signal_exit_code(_status: ExitStatus) -> Option<i32> { None }

#[cfg(test)]
mod tests;
