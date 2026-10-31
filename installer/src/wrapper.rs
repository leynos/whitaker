//! Wrapper scripts generation for easy Whitaker invocation.
//!
//! This module generates platform-specific `whitaker` and `whitaker-ls`
//! scripts. They hold no paths: each one finds `whitaker-installer` (beside
//! itself first, then on `PATH`) and calls `whitaker-installer run`, which
//! resolves the lint libraries, Clippy snapshot, and driver directory from the
//! current data root, serializes cold driver construction, and returns
//! `cargo dylint`'s exit status. A relocated installation therefore needs no
//! regenerated scripts.

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};
use crate::resolution::SUITE_CRATE;
use std::path::Path;

/// Result of wrapper script generation.
#[derive(Debug)]
pub struct WrapperResult {
    /// Conventional path for the `whitaker` wrapper, whether or not that
    /// script was selected for creation.
    pub whitaker_path: std::path::PathBuf,
    /// Path to the `whitaker-ls` wrapper script created by this invocation.
    pub whitaker_ls_path: std::path::PathBuf,
    /// Whether the bin directory is in PATH.
    pub in_path: bool,
}

/// Selects which compatibility scripts an installer invocation may create.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperScripts {
    /// Write both scripts used by the legacy `whitaker-installer` command.
    Both,
    /// Write only the lister script because the `whitaker` name is now a binary.
    WhitakerLsOnly,
}

/// Generates both wrapper scripts for invoking Whitaker lints.
///
/// Creates `whitaker` and `whitaker-ls` scripts (shell on Unix, PowerShell on
/// Windows). `whitaker` forwards its arguments to `whitaker-installer run`,
/// while `whitaker-ls` filters `cargo dylint list` output to the Whitaker
/// suite.
///
/// # Errors
///
/// Returns `InstallerError::WrapperGeneration` if script creation fails.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::wrapper::generate_wrapper_scripts;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialize directories");
/// let result = generate_wrapper_scripts(&dirs)?;
///
/// println!("Script created at: {}", result.whitaker_path.display());
/// if result.in_path {
///     println!("Ready to use: whitaker --all");
/// } else {
///     println!("Add the bin directory to your PATH first");
/// }
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn generate_wrapper_scripts(dirs: &dyn BaseDirs) -> Result<WrapperResult> {
    generate_wrapper_scripts_with(dirs, WrapperScripts::Both)
}

/// Generates the selected compatibility scripts for invoking Whitaker lints.
///
/// The returned paths name the conventional locations even when a script was
/// not selected for writing. This lets callers report a consistent PATH
/// location while preserving an executable installed at the `whitaker` path.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::{
///     dirs::SystemBaseDirs,
///     wrapper::{WrapperScripts, generate_wrapper_scripts_with},
/// };
///
/// let dirs = SystemBaseDirs::new().expect("platform directories should exist");
/// let result =
///     generate_wrapper_scripts_with(&dirs, WrapperScripts::WhitakerLsOnly)?;
/// assert!(result.whitaker_ls_path.exists());
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
///
/// # Errors
///
/// Returns a wrapper-generation error if the bin directory cannot be created
/// or a selected script cannot be written.
pub fn generate_wrapper_scripts_with(
    dirs: &dyn BaseDirs,
    scripts: WrapperScripts,
) -> Result<WrapperResult> {
    let bin_dir = dirs.executables().ok_or_else(|| {
