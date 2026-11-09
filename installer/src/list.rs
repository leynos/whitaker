//! List command implementation.
//!
//! This module provides the `run_list` command handler and supporting functions
//! for querying and displaying installed lint libraries.

use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use log::trace;

use crate::{
    cli::ListArgs,
    dirs::{BaseDirs, SystemBaseDirs},
    error::{InstallerError, Result},
    list_output::{format_human, format_json},
    scanner::InstalledLints,
    stager::default_target_dir,
    toolchain::Toolchain,
};

/// Lists installed lint libraries and their associated lints.
///
/// Scans the staging directory for installed libraries, detects the active
/// toolchain from `rust-toolchain.toml` in the current directory (if present),
/// and formats the output for display.
///
/// Output is written to stdout (human-readable by default, JSON with `--json`).
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use clap::Parser as _;
/// use whitaker_installer::{cli::ListArgs, list::run_list};
///
/// let arguments = ListArgs::try_parse_from(["whitaker-installer"])?;
/// run_list(&arguments, &mut io::stdout())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The staging directory cannot be scanned
/// - Writing to stdout fails
pub fn run_list(args: &ListArgs, stdout: &mut dyn Write) -> Result<()> {
    run_list_with(args, stdout, detect_active_toolchain)
}

/// Collects installed lint metadata without formatting or writing output.
///
/// This is the inventory boundary used by the root `whitaker ls` command.
/// Callers can select their own stable or evolving presentation while sharing
/// the installer's scan-root and ordering rules.
///
/// # Examples
///
/// ```no_run
/// use clap::Parser;
/// use whitaker_installer::{cli::ListArgs, list::collect_installed};
///
/// let arguments = ListArgs::try_parse_from(["whitaker"])?;
/// let installed = collect_installed(&arguments)?;
/// println!(
///     "{} toolchains have staged lints",
///     installed.by_toolchain.len()
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if a selected staging directory cannot be scanned.
pub fn collect_installed(args: &ListArgs) -> Result<crate::scanner::InstalledLints> {
    let scan_roots = determine_scan_roots(args.target_dir.as_deref())?;
    let mut installed = crate::scanner::InstalledLints::default();
    for root in scan_roots {
        let discovered = crate::scanner::scan_installed(&root)
            .map_err(|source| InstallerError::ScanFailed { source })?;
        merge_installed(&mut installed, discovered);
    }
    sort_installed_libraries(&mut installed);
    Ok(installed)
}

/// Internal implementation with injectable toolchain detection for testability.
fn run_list_with<F>(args: &ListArgs, stdout: &mut dyn Write, detect_toolchain: F) -> Result<()>
where
    F: FnOnce() -> Option<String>,
{
    let installed = collect_installed(args)?;

    let active_toolchain = detect_toolchain();

    let output = if args.json {
        format_json(&installed, active_toolchain.as_deref())
    } else {
        format_human(&installed, active_toolchain.as_deref())
    };

    writeln!(stdout, "{output}").map_err(|e| InstallerError::WriteFailed { source: e })?;

    Ok(())
}

fn merge_installed(target: &mut InstalledLints, discovered: InstalledLints) {
    for (toolchain, mut libraries) in discovered.by_toolchain {
        let entry = target.by_toolchain.entry(toolchain).or_default();
        entry.append(&mut libraries);
    }
}

fn sort_installed_libraries(installed: &mut InstalledLints) {
    for libraries in installed.by_toolchain.values_mut() {
        libraries.sort_by(|left, right| left.crate_name.as_str().cmp(right.crate_name.as_str()));
    }
}

fn default_prebuilt_target_dir() -> Option<Utf8PathBuf> {
    SystemBaseDirs::new()
        .and_then(|dirs| dirs.whitaker_data())
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .map(|path| path.join("lints"))
}

fn determine_scan_roots(cli_target: Option<&Utf8Path>) -> Result<Vec<Utf8PathBuf>> {
    if let Some(target) = cli_target {
        return Ok(vec![target.to_owned()]);
    }

    let mut roots = Vec::new();
    if let Some(default) = default_target_dir() {
        roots.push(default);
    }
    if let Some(prebuilt) = default_prebuilt_target_dir()
        && !roots.iter().any(|root| root == &prebuilt)
    {
        roots.push(prebuilt);
    }

    if roots.is_empty() {
        return Err(InstallerError::StagingFailed {
            reason: "could not determine any scan roots".to_owned(),
        });
    }
    Ok(roots)
}

/// Detect the active toolchain, reporting why detection failed.
///
/// Returns `Ok(None)` when the working directory declares no toolchain, which
/// is the normal case outside a checkout. Every other failure is returned so a
/// caller whose behaviour depends on the answer can report it instead of
/// silently falling back to "nothing is installed".
///
/// # Errors
///
/// Returns [`InstallerError::ToolchainDetection`] when the working directory
/// cannot be determined or is not valid UTF-8, and propagates the read or parse
/// failure when `rust-toolchain.toml` exists but cannot be used.
pub fn try_detect_active_toolchain() -> Result<Option<String>> {
    let cwd = std::env::current_dir().map_err(|error| InstallerError::ToolchainDetection {
        reason: format!("could not determine the current directory: {error}"),
    })?;
    let utf8_cwd =
        Utf8PathBuf::try_from(cwd).map_err(|error| InstallerError::ToolchainDetection {
            reason: format!("the current directory is not valid UTF-8: {error}"),
        })?;

    try_detect_active_toolchain_in(&utf8_cwd)
}

/// Detect the active toolchain in a directory, distinguishing absence from
/// failure.
///
/// This is the internal implementation that accepts a path for testability;
/// [`try_detect_active_toolchain`] supplies the working directory.
///
/// # Errors
///
/// Propagates [`InstallerError::Io`] when the toolchain file cannot be read and
/// [`InstallerError::InvalidToolchainFile`] when it cannot be parsed. A missing
/// file is not an error and yields `Ok(None)`.
pub(crate) fn try_detect_active_toolchain_in(dir: &Utf8Path) -> Result<Option<String>> {
    match Toolchain::detect(dir) {
        Ok(toolchain) => Ok(Some(toolchain.channel().to_owned())),
        Err(InstallerError::ToolchainFileNotFound { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Detect the active toolchain, treating every failure as "no toolchain".
///
/// This is for display only: a listing with no active toolchain still shows
/// what was installed. A caller that acts on the result must use
/// [`try_detect_active_toolchain`], which distinguishes an absent toolchain
/// file from one that exists but cannot be read or parsed.
#[must_use]
pub fn detect_active_toolchain() -> Option<String> {
    match try_detect_active_toolchain() {
        Ok(channel) => channel,
        Err(error) => {
            trace!("detect_active_toolchain: toolchain detection failed: {error}");
            None
        }
    }
}

/// Determines the target directory from CLI or falls back to the default.
///
/// If a target directory is provided via CLI, it is used directly. Otherwise,
/// the default staging directory from [`crate::stager::default_target_dir`] is
/// used.
///
/// # Errors
///
/// Returns [`InstallerError::StagingFailed`] if no target directory can be
/// determined (neither provided nor default available).
pub fn determine_target_dir(cli_target: Option<&Utf8Path>) -> Result<Utf8PathBuf> {
    determine_target_dir_with(cli_target, default_target_dir)
}

/// Internal implementation with injectable default provider for testability.
fn determine_target_dir_with<F>(cli_target: Option<&Utf8Path>, default_fn: F) -> Result<Utf8PathBuf>
where
    F: FnOnce() -> Option<Utf8PathBuf>,
{
    cli_target
        .map(Utf8Path::to_owned)
        .or_else(default_fn)
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "could not determine default target directory".to_owned(),
        })
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
