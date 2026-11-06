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

use super::run_install;
use crate::{
    cli::InstallArgs,
    error::Result,
    install_flow::bootstrap_step::run_bootstrap,
    wrapper::WrapperScripts,
};

/// Runs the installed bootstrap for a forwarded `cargo dylint` invocation.
///
/// Returns the child's exit status so a composition root can exit with it
/// unchanged; `whitaker-installer run` forwards lint statuses this way.
pub fn run_bootstrap_command(arguments: &[OsString], stderr: &mut dyn Write) -> i32 {
    run_bootstrap_command_with_library_path(arguments, None, stderr)
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
    stderr: &mut dyn Write,
) -> i32 {
    run_bootstrap(arguments, library_path, stderr)
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
    run_install(args, stdout, stderr, WrapperScripts::None)
}
