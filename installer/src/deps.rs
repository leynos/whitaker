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
    install_missing_tools(
        executor,
        status,
        stderr,
        InstallContext {
            dirs,
            repository_installer: Some(&repository_installer),
            target: target.as_ref(),
            cargo_fallback_mode: InstallMode::CargoInstall,
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
    install_missing_tools(
        executor,
        status,
        stderr,
        InstallContext {
            dirs: Some(options.dirs),
            repository_installer: Some(options.repository_installer),
            target: options.target.as_ref(),
            cargo_fallback_mode: InstallMode::CargoInstall,
            quiet: options.quiet,
        },
    )
}

struct InstallContext<'a> {
    dirs: Option<&'a dyn BaseDirs>,
    repository_installer: Option<&'a dyn DependencyBinaryInstaller>,
    target: Option<&'a TargetTriple>,
    cargo_fallback_mode: InstallMode,
    quiet: bool,
}

fn install_missing_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    stderr: &mut dyn Write,
    context: InstallContext<'_>,
) -> Result<()> {
    let cargo_fallback_mode = if is_binstall_available(executor) {
        InstallMode::Binstall
    } else {
        InstallMode::CargoInstall
    };
    let context = InstallContext {
        cargo_fallback_mode,
        ..context
    };

    for tool in missing_tools(status) {
        install_tool(executor, tool, stderr, &context)?;
    }

    Ok(())
}

fn missing_tools(status: &DylintToolStatus) -> Vec<&'static DependencyTool> {
    let mut tools = Vec::new();
    if !status.cargo_dylint {
        tools.push(&DEPENDENCY_TOOLS[0]);
    }
    if !status.dylint_link {
        tools.push(&DEPENDENCY_TOOLS[1]);
    }
    tools
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
    let Some(target) = context.target else {
        return Ok(false);
    };
    let Some(dirs) = context.dirs else {
        return Ok(false);
    };
    let Some(repository_installer) = context.repository_installer else {
        return Ok(false);
    };
    let Some(dependency) = find_dependency_binary(tool.package) else {
        return Ok(false);
    };

    match repository_installer.install(dependency, target, dirs) {
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

#[derive(Debug, Clone, Copy)]
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
    let args = match context.cargo_fallback_mode {
        InstallMode::Binstall => vec!["binstall", "-y", tool.package],
        InstallMode::CargoInstall => vec!["install", tool.package],
    };
    let output = executor.run("cargo", &args)?;

    if !output.status.success() {
        let message = command_error_message(&output);
        return Err(InstallerError::DependencyInstall {
            tool: tool.package,
            message,
        });
    }

    let mode_message = match context.cargo_fallback_mode {
        InstallMode::Binstall => "cargo binstall",
        InstallMode::CargoInstall => "cargo install",
    };
    write_message(
        stderr,
        context.quiet,
        format!("Installed {} with {mode_message}.", tool.package),
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
