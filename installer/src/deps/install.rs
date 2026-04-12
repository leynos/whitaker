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
    let mut remaining_status = *status;

    for tool in DEPENDENCY_TOOLS.iter() {
        if !should_install_tool(&remaining_status, tool) {
            continue;
        }

        let outcome = install_tool(executor, tool, stderr, context)?;
        mark_tool_installed(&mut remaining_status, tool);
        refresh_source_build_companions(&mut remaining_status, executor, tool, outcome);
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
) -> Result<InstallOutcome> {
    if let Some(outcome) = try_repository_install(executor, tool, stderr, context)? {
        return Ok(outcome);
    }

    install_with_cargo(executor, tool, stderr, context)
}

pub(super) fn try_repository_install(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<Option<InstallOutcome>> {
    let Some(repo) = &context.repo else {
        return Ok(None);
    };
    let Some(dependency) = find_dependency_binary(tool.package).map_err(|error| {
        InstallerError::DependencyInstall {
            tool: tool.package,
            message: error.to_string(),
        }
    })?
    else {
        return Ok(None);
    };

    match repo.installer.install(dependency, repo.target, repo.dirs) {
        Ok(_) if is_tool_installed(executor, tool) => {
            write_message(
                stderr,
                context.quiet,
                format!("Installed {} from repository release.", tool.package),
            );
            Ok(Some(InstallOutcome::Installed))
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
            Ok(None)
        }
        Err(error) if error.is_not_found() => {
            write_message(
                stderr,
                context.quiet,
                format!(
                    "Repository install for {} unavailable: {error}. Building from source with Cargo.",
                    tool.package
                ),
            );
            run_cargo_install(
                executor,
                stderr,
                context,
                cargo_source_install_request(tool, dependency),
            )
            .map(Some)
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
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstallMode {
    Binstall,
    CargoInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstallOutcome {
    Installed,
    SourceBuild,
}

struct CargoInstallRequest<'a> {
    tool: &'a DependencyTool,
    args: Vec<&'a str>,
    success_message: String,
}

pub(super) fn install_with_cargo(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<InstallOutcome> {
    // Always try binstall first if available; fall back to cargo install on failure
    if context.cargo_fallback_mode == InstallMode::Binstall {
        if try_binstall(executor, tool, stderr, context)? {
            return Ok(InstallOutcome::Installed);
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
) -> Result<InstallOutcome> {
    run_cargo_install(executor, stderr, context, cargo_install_request(tool))
}

fn run_cargo_install(
    executor: &dyn CommandExecutor,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
    request: CargoInstallRequest<'_>,
) -> Result<InstallOutcome> {
    let output = executor.run("cargo", &request.args)?;
    if !output.status.success() {
        let message = command_error_message(&output);
        return Err(InstallerError::DependencyInstall {
            tool: request.tool.package,
            message,
        });
    }

    if !is_tool_installed(executor, request.tool) {
        return Err(InstallerError::DependencyInstall {
            tool: request.tool.package,
            message: format!(
                "cargo install reported success, but {} is still unavailable",
                request.tool.package
            ),
        });
    }

    write_message(stderr, context.quiet, request.success_message);
    Ok(InstallOutcome::SourceBuild)
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

fn mark_tool_installed(status: &mut DylintToolStatus, tool: &DependencyTool) {
    match tool.package {
        "cargo-dylint" => status.cargo_dylint = true,
        "dylint-link" => status.dylint_link = true,
        _ => {}
    }
}

fn should_refresh_companions(
    outcome: InstallOutcome,
    tool: &DependencyTool,
    status: &DylintToolStatus,
) -> bool {
    outcome == InstallOutcome::SourceBuild && tool.package == "cargo-dylint" && !status.dylint_link
}

fn refresh_source_build_companions(
    status: &mut DylintToolStatus,
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    outcome: InstallOutcome,
) {
    if !should_refresh_companions(outcome, tool, status) {
        return;
    }

    // A source build of cargo-dylint can also provide dylint-link.
    status.dylint_link = is_tool_installed(executor, &DEPENDENCY_TOOLS[1]);
}

fn cargo_install_request(tool: &DependencyTool) -> CargoInstallRequest<'_> {
    CargoInstallRequest {
        tool,
        args: vec!["install", tool.package],
        success_message: format!("Installed {} with cargo install.", tool.package),
    }
}

fn cargo_source_install_request<'a>(
    tool: &'a DependencyTool,
    dependency: &'a crate::dependency_binaries::DependencyBinary,
) -> CargoInstallRequest<'a> {
    CargoInstallRequest {
        tool,
        args: vec![
            "install",
            "--locked",
            "--version",
            dependency.version(),
            tool.package,
        ],
        success_message: format!("Installed {} from source with cargo install.", tool.package),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_refresh_companions_requires_source_built_cargo_dylint_without_link() {
        let cargo_dylint = &DEPENDENCY_TOOLS[0];
        let dylint_link = &DEPENDENCY_TOOLS[1];
        let missing_link = DylintToolStatus {
            cargo_dylint: true,
            dylint_link: false,
        };
        let installed_link = DylintToolStatus {
            cargo_dylint: true,
            dylint_link: true,
        };

        assert!(should_refresh_companions(
            InstallOutcome::SourceBuild,
            cargo_dylint,
            &missing_link,
        ));
        assert!(!should_refresh_companions(
            InstallOutcome::Installed,
            cargo_dylint,
            &missing_link,
        ));
        assert!(!should_refresh_companions(
            InstallOutcome::SourceBuild,
            dylint_link,
            &missing_link,
        ));
        assert!(!should_refresh_companions(
            InstallOutcome::SourceBuild,
            cargo_dylint,
            &installed_link,
        ));
    }
}
