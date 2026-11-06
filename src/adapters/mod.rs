//! Production adapters for installer, inventory, and Dylint subprocess ports.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    process::{Command, ExitStatus},
};

use camino::Utf8PathBuf;
use whitaker_installer::{
    bootstrap::is_installed,
    dependency_binaries::host_target,
    dirs::{BaseDirs, SystemBaseDirs},
    installation_record::installed_paths,
    list::{collect_installed, detect_active_toolchain},
    orchestration::{run_bootstrap_command_with_library_path, run_install_for_whitaker},
    prebuilt_path::prebuilt_library_dir,
    stager::default_target_dir,
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
        let library_path = active_library_path(dirs)?;
        if has_installed_bootstrap(dirs) {
            return Ok(run_bootstrap_command_with_library_path(
                args,
                library_path.as_deref(),
                stderr,
            ));
        }
        forward_without_bootstrap(args, library_path.as_deref())
    }
}

/// Report whether an installation recorded a bootstrap to run from.
fn has_installed_bootstrap(dirs: Option<&dyn BaseDirs>) -> bool { dirs.is_some_and(is_installed) }

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

#[cfg(unix)]
fn signal_exit_code(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal().map(|signal| 128 + signal)
}

#[cfg(not(unix))]
const fn signal_exit_code(_status: ExitStatus) -> Option<i32> { None }

/// Resolves the library path used when forwarding a command to cargo-dylint.
///
/// # Errors
///
/// Returns [`CliError::InstallationRecord`] when an installation record exists
/// but cannot be locked, read, or parsed. A missing record is not an error: it
/// simply means no toolchain has been installed yet.
fn active_library_path(dirs: Option<&dyn BaseDirs>) -> Result<Option<OsString>, CliError> {
    let explicit_path = std::env::var_os("DYLINT_LIBRARY_PATH");
    if explicit_path.as_ref().is_some_and(|path| !path.is_empty()) {
        return Ok(explicit_path);
    }
    let recorded_paths = recorded_library_paths(dirs)?;
    let active_toolchain = detect_active_toolchain();
    let build_path = active_toolchain.as_ref().and_then(|toolchain| {
        default_target_dir().map(|target_dir| target_dir.join(toolchain).join("release"))
    });
    let prebuilt_path = active_toolchain
        .as_deref()
        .and_then(|toolchain| prebuilt_library_path(dirs, toolchain));
    Ok(active_library_path_for(
        explicit_path,
        recorded_paths,
        build_path,
        prebuilt_path,
    ))
}

/// Reads the per-toolchain staging directories recorded by earlier installs.
///
/// An unavailable platform data directory yields no recorded paths, matching
/// the "nothing installed" case. A record that exists but fails to load is
/// propagated so the caller can report it instead of silently forwarding with
/// an empty library path.
///
/// The directory provider is a parameter so callers pass one resolved view,
/// and so the record's error paths can be exercised with a directory a test
/// owns. An environment-driven redirect is not portable: Windows resolves the
/// data directory from the known-folder API and ignores `XDG_DATA_HOME`.
fn recorded_library_paths(dirs: Option<&dyn BaseDirs>) -> Result<Vec<Utf8PathBuf>, CliError> {
    dirs.map_or_else(
        || Ok(Vec::new()),
        |provider| {
            installed_paths(provider).map_err(|source| CliError::InstallationRecord { source })
        },
    )
}

/// Includes recorded and default directories so Dylint can select by toolchain.
fn active_library_path_for(
    explicit_path: Option<OsString>,
    recorded_paths: Vec<Utf8PathBuf>,
    build_path: Option<Utf8PathBuf>,
    prebuilt_path: Option<Utf8PathBuf>,
) -> Option<OsString> {
    if let Some(path) = explicit_path.filter(|path| !path.is_empty()) {
        return Some(path);
    }
    let mut candidates = recorded_paths;
    for candidate in [build_path, prebuilt_path].into_iter().flatten() {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    existing_recorded_directories(candidates)
}

/// Joins every existing recorded path so Dylint can select by toolchain.
fn existing_recorded_directories(paths: Vec<Utf8PathBuf>) -> Option<OsString> {
    let existing: Vec<_> = paths
        .into_iter()
        .filter(|path| path.is_absolute() && path.is_dir())
        .map(Utf8PathBuf::into_std_path_buf)
        .filter(|path| std::env::join_paths([path]).is_ok())
        .collect();
    (!existing.is_empty())
        .then(|| std::env::join_paths(existing).ok())
        .flatten()
}

/// Resolves the prebuilt library directory for the active host and toolchain.
fn prebuilt_library_path(dirs: Option<&dyn BaseDirs>, toolchain: &str) -> Option<Utf8PathBuf> {
    let target = host_target()?;
    prebuilt_library_dir(dirs?, toolchain, target.as_str()).ok()
}

#[cfg(test)]
mod tests;
