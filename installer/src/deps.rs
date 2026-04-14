//! Dependency installation for Dylint tools.
//!
//! This module checks whether `cargo-dylint` and `dylint-link` are already
//! available, then installs any missing tools by preferring repository-hosted
//! release archives before falling back to `cargo binstall` or `cargo install`.

use crate::dependency_binaries::{
    DependencyBinaryInstaller, RepositoryDependencyBinaryInstaller, host_target,
};
use crate::dirs::{BaseDirs, SystemBaseDirs};
use crate::error::{InstallerError, Result};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

mod install;
use install::*;

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
    pub fn all_installed(&self) -> bool {
        self.cargo_dylint && self.dylint_link
    }
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
    /// Whether stderr output should be suppressed.
    pub quiet: bool,
}

/// Checks whether the Dylint tools are installed.
#[must_use]
pub fn check_dylint_tools(executor: &dyn CommandExecutor) -> DylintToolStatus {
    DylintToolStatus {
        cargo_dylint: is_tool_installed(executor, &CARGO_DYLINT_TOOL),
        dylint_link: is_tool_installed(executor, &DYLINT_LINK_TOOL),
    }
}

/// Install missing tools without emitting progress output.
pub fn install_dylint_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
) -> Result<()> {
    let mut sink = io::sink();
    install_dylint_tools_with_output(executor, status, true, &mut sink)
}

/// Install missing tools while writing progress output to `stderr`.
pub fn install_dylint_tools_with_output(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    quiet: bool,
    stderr: &mut dyn Write,
) -> Result<()> {
    let repository_installer = RepositoryDependencyBinaryInstaller;
    let system_dirs = SystemBaseDirs::new();
    let target = host_target();
    let dirs = system_dirs.as_ref().map(|dirs| dirs as &dyn BaseDirs);
    let cargo_fallback_mode = cargo_fallback_mode(executor);
    install_missing_tools(
        executor,
        status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                dirs,
                Some(&repository_installer as &dyn DependencyBinaryInstaller),
                target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet,
        },
    )
}

/// Install missing tools with injected repository-install hooks.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn install_dylint_tools_with_options(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    stderr: &mut dyn Write,
    options: DependencyInstallOptions<'_>,
) -> Result<()> {
    let cargo_fallback_mode = cargo_fallback_mode(executor);
    install_missing_tools(
        executor,
        status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                Some(options.dirs),
                Some(options.repository_installer),
                options.target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet: options.quiet,
        },
    )
}

fn is_tool_installed(executor: &dyn CommandExecutor, tool: &DependencyTool) -> bool {
    if tool == &DYLINT_LINK_TOOL {
        if !is_binary_on_path(tool.command) {
            return false;
        }

        return find_binary_on_path(tool.command)
            .is_some_and(|binary_path| dylint_link_probe_succeeds(&binary_path));
    }
    command_succeeds(executor, tool.command, tool.args)
}

fn is_binstall_available(executor: &dyn CommandExecutor) -> bool {
    command_succeeds(executor, "cargo", &["binstall", "--version"])
}

fn is_binary_on_path(binary_name: &str) -> bool {
    find_binary_on_path(binary_name).is_some()
}

fn find_binary_on_path(binary_name: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH")?;

    std::env::split_paths(&path_var)
        .find_map(|directory| find_binary_in_directory(&directory, binary_name))
}

fn find_binary_in_directory(directory: &Path, binary_name: &str) -> Option<std::path::PathBuf> {
    binary_candidates(directory, binary_name)
        .into_iter()
        .find(|candidate| is_executable_file(candidate))
}

fn dylint_link_probe_succeeds(binary_path: &Path) -> bool {
    let mut command = Command::new(binary_path);
    command.arg("--help");

    if let Some(toolchain) = dylint_link_probe_toolchain() {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }

    command.output().is_ok_and(|output| output.status.success())
}

fn dylint_link_probe_toolchain() -> Option<String> {
    std::env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .filter(|toolchain| !toolchain.trim().is_empty())
        .or_else(|| {
            host_target().map(|target| {
                // `dylint-link` reads `RUSTUP_TOOLCHAIN` before it inspects CLI
                // arguments, so the probe synthesizes a stable host toolchain
                // when the caller did not provide one.
                format!("stable-{}", target.as_str())
            })
        })
}

fn binary_candidates(directory: &Path, binary_name: &str) -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    let mut candidates = Vec::new();
    #[cfg(not(windows))]
    let candidates = vec![directory.join(binary_name)];
    #[cfg(windows)]
    {
        if Path::new(binary_name).extension().is_some() {
            candidates.push(directory.join(binary_name));
        }
        let lowercase_name = binary_name.to_ascii_lowercase();
        candidates.extend(
            windows_path_extensions()
                .into_iter()
                .filter(|extension| !lowercase_name.ends_with(&extension.to_ascii_lowercase()))
                .map(|extension| directory.join(format!("{binary_name}{extension}"))),
        );
    }
    candidates
}

#[cfg(windows)]
fn windows_path_extensions() -> Vec<String> {
    let path_ext = std::env::var_os("PATHEXT")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| std::ffi::OsString::from(".COM;.EXE;.BAT;.CMD"));

    path_ext
        .to_string_lossy()
        .split(';')
        .filter_map(|extension| {
            let trimmed = extension.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.starts_with('.') {
                Some(trimmed.to_owned())
            } else {
                Some(format!(".{trimmed}"))
            }
        })
        .collect()
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod tests;
