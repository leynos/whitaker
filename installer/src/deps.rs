//! Dependency installation for Dylint tools.
//!
//! This module checks whether `cargo-dylint` and `dylint-link` are already
//! available, then installs any missing tools by preferring repository-hosted
//! release archives before falling back to `cargo binstall` or `cargo install`.

use crate::dependency_binaries::{
    DependencyBinaryInstaller, RepositoryDependencyBinaryInstaller, find_dependency_binary,
    host_target,
};
use crate::dirs::{BaseDirs, SystemBaseDirs};
use crate::error::{InstallerError, Result};
use crate::installer_packaging::TargetTriple;
use std::io;
use std::io::Write;
use std::process::{Command, Output};

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

#[derive(Debug, Clone, Copy)]
struct DependencyTool {
    package: &'static str,
    command: &'static str,
    args: &'static [&'static str],
}

const DEPENDENCY_TOOLS: [DependencyTool; 2] = [
    DependencyTool {
        package: "cargo-dylint",
        command: "cargo",
        args: &["dylint", "--version"],
    },
    DependencyTool {
        package: "dylint-link",
        command: "dylint-link",
        args: &["--version"],
    },
];

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
    pub target: Option<TargetTriple>,
    /// Whether stderr output should be suppressed.
    pub quiet: bool,
}

/// Checks whether the Dylint tools are installed.
#[must_use]
pub fn check_dylint_tools(executor: &dyn CommandExecutor) -> DylintToolStatus {
    DylintToolStatus {
        cargo_dylint: is_tool_installed(executor, &DEPENDENCY_TOOLS[0]),
        dylint_link: is_tool_installed(executor, &DEPENDENCY_TOOLS[1]),
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

struct RepositoryInstallContext<'a> {
    dirs: &'a dyn BaseDirs,
    installer: &'a dyn DependencyBinaryInstaller,
    target: &'a TargetTriple,
}

struct InstallContext<'a> {
    repo: Option<RepositoryInstallContext<'a>>,
    cargo_fallback_mode: InstallMode,
    quiet: bool,
}

fn install_missing_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    for tool in missing_tools(status) {
        install_tool(executor, tool, stderr, context)?;
    }

    Ok(())
}

fn repository_install_context<'a>(
    dirs: Option<&'a dyn BaseDirs>,
    installer: Option<&'a dyn DependencyBinaryInstaller>,
    target: Option<&'a TargetTriple>,
) -> Option<RepositoryInstallContext<'a>> {
    match (dirs, installer, target) {
        (Some(dirs), Some(installer), Some(target)) => Some(RepositoryInstallContext {
            dirs,
            installer,
            target,
        }),
        _ => None,
    }
}

fn cargo_fallback_mode(executor: &dyn CommandExecutor) -> InstallMode {
    if is_binstall_available(executor) {
        InstallMode::Binstall
    } else {
        InstallMode::CargoInstall
    }
}

fn missing_tools(status: &DylintToolStatus) -> impl Iterator<Item = &'static DependencyTool> + '_ {
    DEPENDENCY_TOOLS
        .iter()
        .filter(move |tool| should_install_tool(status, tool))
}

fn should_install_tool(status: &DylintToolStatus, tool: &DependencyTool) -> bool {
    match tool.package {
        "cargo-dylint" => !status.cargo_dylint,
        "dylint-link" => !status.dylint_link,
        _ => false,
    }
}

fn is_tool_installed(executor: &dyn CommandExecutor, tool: &DependencyTool) -> bool {
    command_succeeds(executor, tool.command, tool.args)
}

fn is_binstall_available(executor: &dyn CommandExecutor) -> bool {
    command_succeeds(executor, "cargo", &["binstall", "--version"])
}

fn install_tool(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    if try_repository_install(executor, tool, stderr, context)? {
        return Ok(());
    }

    install_with_cargo(executor, tool, stderr, context)
}

fn try_repository_install(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<bool> {
    let Some(repo) = &context.repo else {
        return Ok(false);
    };
    let Some(dependency) = find_dependency_binary(tool.package).map_err(|error| {
        InstallerError::DependencyInstall {
            tool: tool.package,
            message: error.to_string(),
        }
    })?
    else {
        return Ok(false);
    };

    match repo.installer.install(dependency, repo.target, repo.dirs) {
        Ok(_) if is_tool_installed(executor, tool) => {
            write_message(
                stderr,
                context.quiet,
                format!("Installed {} from repository release.", tool.package),
            );
            Ok(true)
        }
        Ok(_) => {
            write_message(
                stderr,
                context.quiet,
                format!(
                    "Repository install for {} failed verification; falling back to Cargo.",
                    tool.package
                ),
            );
            Ok(false)
        }
        Err(error) => {
            write_message(
                stderr,
                context.quiet,
                format!(
                    "Repository install for {} unavailable: {error}. Falling back to Cargo.",
                    tool.package
                ),
            );
            Ok(false)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallMode {
    Binstall,
    CargoInstall,
}

fn install_with_cargo(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    // Always try binstall first if available; fall back to cargo install on failure
    if context.cargo_fallback_mode == InstallMode::Binstall {
        if try_binstall(executor, tool, stderr, context)? {
            return Ok(());
        }
        write_message(
            stderr,
            context.quiet,
            format!(
                "cargo binstall failed for {}; falling back to cargo install.",
                tool.package
            ),
        );
    }

    try_cargo_install(executor, tool, stderr, context)
}

fn try_binstall(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<bool> {
    let args = vec!["binstall", "-y", tool.package];
    let output = executor.run("cargo", &args)?;

    if !output.status.success() {
        return Ok(false);
    }

    if !is_tool_installed(executor, tool) {
        return Ok(false);
    }

    write_message(
        stderr,
        context.quiet,
        format!("Installed {} with cargo binstall.", tool.package),
    );
    Ok(true)
}

fn try_cargo_install(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    let args = vec!["install", tool.package];
    let output = executor.run("cargo", &args)?;

    if !output.status.success() {
        let message = command_error_message(&output);
        return Err(InstallerError::DependencyInstall {
            tool: tool.package,
            message,
        });
    }

    if !is_tool_installed(executor, tool) {
        return Err(InstallerError::DependencyInstall {
            tool: tool.package,
            message: format!(
                "cargo install reported success, but {} is still unavailable",
                tool.package
            ),
        });
    }

    write_message(
        stderr,
        context.quiet,
        format!("Installed {} with cargo install.", tool.package),
    );
    Ok(())
}

fn command_error_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        format!("command exited with {}", output.status)
    } else {
        trimmed.to_owned()
    }
}

fn write_message(stderr: &mut dyn Write, quiet: bool, message: String) {
    if quiet {
        return;
    }
    let _ = writeln!(stderr, "{message}");
}

fn command_succeeds(executor: &dyn CommandExecutor, cmd: &str, args: &[&str]) -> bool {
    executor
        .run(cmd, args)
        .is_ok_and(|output| output.status.success())
}

#[cfg(test)]
mod tests;
