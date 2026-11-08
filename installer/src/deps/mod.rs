//! Dependency installation for Dylint tools.
//!
//! This module checks whether `cargo-dylint` and `dylint-link` are already
//! available, then installs any missing tools by preferring repository-hosted
//! release archives before falling back to `cargo binstall` or `cargo install`.

use std::{
    io::{self, Write},
    process::{Command, Output},
};

use crate::{
    dependency_binaries::{
        DependencyBinaryInstaller,
        RepositoryDependencyBinaryInstaller,
        host_target,
    },
    dirs::{BaseDirs, SystemBaseDirs},
    error::{InstallerError, Result},
};

mod install;
mod install_cargo;
mod install_repository;
use install::{
    InstallContext,
    RepositoryInstallContext,
    cargo_fallback_mode,
    install_missing_tools,
    repository_install_context,
};

/// Abstraction for running external commands.
pub trait CommandExecutor {
    /// Runs a command with arguments and returns the captured output.
    ///
    /// # Errors
    ///
    /// Returns any I/O errors encountered while spawning or running the command.
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>;
}

/// Executes commands on the host system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        Command::new(cmd)
            .args(args)
            .output()
            .map_err(InstallerError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DependencyTool {
    package: &'static str,
    command: &'static str,
    args: &'static [&'static str],
}

const CARGO_DYLINT_TOOL: DependencyTool = DependencyTool {
    package: "cargo-dylint",
    command: "cargo",
    args: &["dylint", "--version"],
};

const DYLINT_LINK_TOOL: DependencyTool = DependencyTool {
    package: "dylint-link",
    command: "dylint-link",
    args: &["--version"],
};

const DEPENDENCY_TOOLS: [DependencyTool; 2] = [CARGO_DYLINT_TOOL, DYLINT_LINK_TOOL];

#[path = "availability.rs"]
mod availability;
#[path = "path_scan.rs"]
mod path_scan;

// `pub(super)` inside `availability` already reaches `deps`, so the names are
// visible to siblings without a wider re-export.
#[cfg(test)]
pub(crate) use availability::check_dylint_tools_with_locator;
pub use availability::{check_dylint_tools, check_dylint_tools_with_diagnostics};
// Private re-export: `pub(super)` inside `availability` reaches `deps`, and a
// private binding here is in turn visible to `deps`'s descendant modules.
use availability::{is_binstall_available, is_tool_installed};
// Only the sibling test modules name the trait directly; the production path
// reaches it through `availability`'s own `use`.
#[cfg(test)]
pub(crate) use path_scan::BinaryLocator;
#[cfg(test)]
pub(crate) use path_scan::PathScanResult;
pub(crate) use path_scan::SystemBinaryLocator;
pub use path_scan::{PathScanFailure, PathScanFailureCategory};

/// Install missing tools without emitting progress output.
///
/// # Errors
///
/// Returns query, installation, or source-policy errors from the install flow.
pub fn install_dylint_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
) -> Result<()> {
    let mut sink = io::sink();
    install_dylint_tools_with_output(
        executor,
        status,
        SourcePolicy {
            quiet: true,
            no_source_fallback: false,
        },
        &mut sink,
    )
}

/// Install missing tools while writing progress output to `stderr`.
///
/// # Errors
///
/// Returns query, installation, or source-policy errors from the install flow.
pub fn install_dylint_tools_with_output(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    policy: SourcePolicy,
    stderr: &mut dyn Write,
) -> Result<()> {
    let repository_installer = RepositoryDependencyBinaryInstaller;
    let system_dirs = SystemBaseDirs::new();
    let target = host_target();
    let dirs = system_dirs.as_ref().map(|dirs| dirs as &dyn BaseDirs);
    let cargo_fallback_mode = cargo_fallback_mode(executor)?;
    install_missing_tools(
        executor,
        *status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                dirs,
                Some(&repository_installer as &dyn DependencyBinaryInstaller),
                target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet: policy.quiet,
            no_source_fallback: policy.no_source_fallback,
        },
    )
}

/// Install missing tools with injected repository-install hooks.
///
/// # Errors
///
/// Returns query, installation, or source-policy errors from the install flow.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn install_dylint_tools_with_options(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    stderr: &mut dyn Write,
    options: DependencyInstallOptions<'_>,
) -> Result<()> {
    let DependencyInstallOptions {
        dirs,
        repository_installer,
        target,
        policy,
    } = options;
    let cargo_fallback_mode = cargo_fallback_mode(executor)?;
    install_missing_tools(
        executor,
        *status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                Some(dirs),
                Some(repository_installer),
                target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet: policy.quiet,
            no_source_fallback: policy.no_source_fallback,
        },
    )
}

/// Status and diagnostics from a successful availability query.
#[derive(Debug)]
#[doc(hidden)]
pub struct DylintToolCheck {
    /// Availability for each required tool.
    pub status: DylintToolStatus,
    /// PATH entries skipped before a match was found.
    pub path_scan_failures: Vec<PathScanFailure>,
}

/// Status of Dylint tool availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DylintToolStatus {
    /// Whether `cargo-dylint` is installed.
    pub cargo_dylint: bool,
    /// Whether `dylint-link` is installed.
    pub dylint_link: bool,
}

impl DylintToolStatus {
    /// Returns `true` when both tools are installed.
    #[must_use]
    pub const fn all_installed(&self) -> bool { self.cargo_dylint && self.dylint_link }
}

/// How a run should react when a published artefact is missing.
///
/// Carried together because they travel together: every layer that decides
/// whether to print also decides whether a source build is acceptable, and
/// threading two booleans through the same five signatures invites them to
/// drift apart.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourcePolicy {
    /// Whether stderr output should be suppressed.
    pub quiet: bool,
    /// Whether a missing artefact is an error rather than a source build.
    pub no_source_fallback: bool,
}

/// Additional install options used by test-support hooks.
#[cfg(any(test, feature = "test-support"))]
pub struct DependencyInstallOptions<'a> {
    /// Base directories used by the repository installer.
    pub dirs: &'a dyn BaseDirs,
    /// The repository-first installer implementation.
    pub repository_installer: &'a dyn DependencyBinaryInstaller,
    /// Host target override used for repository asset naming.
    pub target: Option<crate::installer_packaging::TargetTriple>,
    /// How the run reacts to a missing published artefact.
    pub policy: SourcePolicy,
}

#[cfg(test)]
mod availability_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod tests;
