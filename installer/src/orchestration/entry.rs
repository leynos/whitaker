//! Entry points used by the root `whitaker` binary.
//!
//! These coordinate an installed bootstrap run and an install that preserves
//! the invoked executable name. They live apart from the rest of the
//! orchestration module so that module stays within its size budget while the
//! root-specific contract remains in one readable place.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

use super::{run_install, with_install_lock};
use crate::{
    bootstrap::RunOptions,
    cli::InstallArgs,
    dirs::{BaseDirs, SystemBaseDirs},
    error::{InstallerError, Result},
    install_flow::bootstrap_step::{run_bootstrap, run_bootstrap_with_dirs},
    wrapper::WrapperScripts,
};

/// Runs the installed bootstrap for a forwarded `cargo dylint` invocation.
///
/// Returns the child's exit status so a composition root can exit with it
/// unchanged; `whitaker-installer run` forwards lint statuses this way.
///
/// `inherited_path` is the caller's own `PATH`, read by the composition root;
/// see [`RunOptions::inherited_path`](crate::bootstrap::RunOptions::inherited_path).
pub fn run_bootstrap_command(
    arguments: &[OsString],
    inherited_path: Option<&OsStr>,
    stderr: &mut dyn Write,
) -> i32 {
    run_bootstrap_command_with_library_path(arguments, None, inherited_path, stderr)
}

/// [`run_bootstrap_command`] with a caller-supplied library path.
///
/// The root CLI records one staging directory per toolchain, which may cover
/// more toolchains than the installation's own record. Passing them here
/// keeps that list authoritative for `DYLINT_LIBRARY_PATH` while the
/// bootstrap still supplies the Clippy snapshot, the driver slot, and the
/// cold-build serialization.
pub fn run_bootstrap_command_with_library_path(
    arguments: &[OsString],
    library_path: Option<&OsStr>,
    inherited_path: Option<&OsStr>,
    stderr: &mut dyn Write,
) -> i32 {
    run_bootstrap(arguments, library_path, inherited_path, stderr)
}

/// [`run_bootstrap_command_with_library_path`] against supplied directories.
///
/// The forwarding adapter resolves the platform directories once per
/// invocation and decides the route from them. This entry point lets a caller
/// that already holds that view drive the same run, so a test can prove that
/// the route it selected is the one that reports a broken installation. It is
/// the only supported way to run the bootstrap against directories other than
/// the host's own; production callers use
/// [`run_bootstrap_command_with_library_path`].
pub fn run_bootstrap_command_with_dirs(
    dirs: &dyn BaseDirs,
    arguments: &[OsString],
    options: RunOptions<'_>,
    stderr: &mut dyn Write,
) -> i32 {
    run_bootstrap_with_dirs(dirs, arguments, options, stderr)
}

/// Runs a root-CLI install without replacing its `whitaker` executable.
///
/// The legacy installer continues to use [`super::run`] and creates both
/// scripts. The root binary creates none: it is itself the `whitaker`
/// executable, and its own `ls` subcommand replaces `whitaker-ls`, whose
/// generated script would need a separately installed `whitaker-installer`.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use whitaker_installer::{cli::InstallArgs, orchestration::run_install_for_whitaker};
///
/// let arguments = InstallArgs::default();
/// let mut output = io::stdout();
/// let mut diagnostics = io::stderr();
/// run_install_for_whitaker(&arguments, &mut output, &mut diagnostics)?;
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
///
/// # Errors
///
/// Returns an error when the selected installation operation cannot complete.
pub fn run_install_for_whitaker(
    args: &InstallArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<()> {
    let dirs = install_dirs()?;
    with_install_lock(&dirs, || {
        run_install(args, stdout, stderr, WrapperScripts::None)
    })?
}

/// Runs a root-CLI install for a caller that already holds the install lock.
///
/// `whitaker check` repairs inside [`with_install_lock`], so taking the lock
/// again here would deadlock against its own handle: the lock is exclusive and
/// is not re-entrant across file handles.
///
/// # Errors
///
/// Returns an error when the selected installation operation cannot complete.
pub fn run_install_for_whitaker_under_lock(
    args: &InstallArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<()> {
    run_install(args, stdout, stderr, WrapperScripts::None)
}

/// Resolve the platform directories an installation writes to.
///
/// # Errors
///
/// Returns [`InstallerError::WorkspaceNotFound`] when the platform provides no
/// data directory, which an install cannot work without.
pub(crate) fn install_dirs() -> Result<SystemBaseDirs> {
    SystemBaseDirs::new().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine platform directories".to_owned(),
    })
}
