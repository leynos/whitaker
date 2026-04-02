//! Install orchestration types and helpers for dependency tools.

use crate::dependency_binaries::{DependencyBinaryInstaller, find_dependency_binary};
use crate::error::{InstallerError, Result};
use crate::installer_packaging::TargetTriple;
use std::io::Write;
use std::process::Output;

use super::{
    CommandExecutor, DEPENDENCY_TOOLS, DependencyTool, DylintToolStatus, is_binstall_available,
    is_tool_installed,
};

pub(super) struct RepositoryInstallContext<'a> {
    pub(super) dirs: &'a dyn crate::dirs::BaseDirs,
    pub(super) installer: &'a dyn DependencyBinaryInstaller,
    pub(super) target: &'a TargetTriple,
}

pub(super) struct InstallContext<'a> {
    pub(super) repo: Option<RepositoryInstallContext<'a>>,
    pub(super) cargo_fallback_mode: InstallMode,
    pub(super) quiet: bool,
}

pub(super) fn install_missing_tools(
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

pub(super) fn repository_install_context<'a>(
    dirs: Option<&'a dyn crate::dirs::BaseDirs>,
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

pub(super) fn cargo_fallback_mode(executor: &dyn CommandExecutor) -> InstallMode {
    if is_binstall_available(executor) {
        InstallMode::Binstall
    } else {
        InstallMode::CargoInstall
    }
}

pub(super) fn missing_tools(
    status: &DylintToolStatus,
) -> impl Iterator<Item = &'static DependencyTool> + '_ {
    DEPENDENCY_TOOLS
        .iter()
        .filter(move |tool| should_install_tool(status, tool))
}

pub(super) fn should_install_tool(status: &DylintToolStatus, tool: &DependencyTool) -> bool {
    match tool.package {
        "cargo-dylint" => !status.cargo_dylint,
        "dylint-link" => !status.dylint_link,
        _ => false,
    }
}

pub(super) fn install_tool(
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

pub(super) fn try_repository_install(
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
pub(super) enum InstallMode {
    Binstall,
    CargoInstall,
}

pub(super) fn install_with_cargo(
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

pub(super) fn try_binstall(
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

pub(super) fn try_cargo_install(
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

pub(super) fn command_error_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        format!("command exited with {}", output.status)
    } else {
        trimmed.to_owned()
    }
}

pub(super) fn write_message(stderr: &mut dyn Write, quiet: bool, message: String) {
    if quiet {
        return;
    }
    let _ = writeln!(stderr, "{message}");
}

pub(super) fn command_succeeds(executor: &dyn CommandExecutor, cmd: &str, args: &[&str]) -> bool {
    executor
        .run(cmd, args)
        .is_ok_and(|output| output.status.success())
}
