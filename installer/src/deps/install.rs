//! Install orchestration types and helpers for dependency tools.

use crate::dependency_binaries::{DependencyBinaryInstaller, find_dependency_binary};
use crate::error::{InstallerError, Result};
use crate::installer_packaging::TargetTriple;
use std::io::Write;
use std::process::Output;

use super::{
    CARGO_DYLINT_TOOL, CommandExecutor, DEPENDENCY_TOOLS, DYLINT_LINK_TOOL, DependencyTool,
    DylintToolStatus, is_binstall_available, is_tool_installed,
};

#[path = "install_repository.rs"]
mod repository;

use repository::{
    RepositoryInstall, RepositoryInstallRequest, attempt_repository_install,
    resolve_dependency_binary,
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
    status: DylintToolStatus,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    let mut remaining_status = status;

    for tool in &DEPENDENCY_TOOLS {
        if !should_install_tool(remaining_status, tool) {
            continue;
        }

        let outcome = install_tool(executor, tool, stderr, context)?;
        update_status_after_install(&mut remaining_status, executor, tool, outcome);
    }

    Ok(())
}

pub(super) fn repository_install_context<'a>(
    dirs: Option<&'a dyn crate::dirs::BaseDirs>,
    installer: Option<&'a dyn DependencyBinaryInstaller>,
    target: Option<&'a TargetTriple>,
) -> Option<RepositoryInstallContext<'a>> {
    match (dirs, installer, target) {
        (Some(base_dirs), Some(binary_installer), Some(target_triple)) => {
            Some(RepositoryInstallContext {
                dirs: base_dirs,
                installer: binary_installer,
                target: target_triple,
            })
        }
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

pub(super) fn should_install_tool(status: DylintToolStatus, tool: &DependencyTool) -> bool {
    if tool == &CARGO_DYLINT_TOOL {
        !status.cargo_dylint
    } else if tool == &DYLINT_LINK_TOOL {
        !status.dylint_link
    } else {
        false
    }
}

pub(super) fn install_tool(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<InstallOutcome> {
    let mut cargo_install_plan = CargoInstallPlan::new(tool);

    if let Some(repo) = &context.repo {
        let dependency = resolve_dependency_binary(tool)?;
        cargo_install_plan = cargo_install_plan.with_version(dependency.version());

        let request = RepositoryInstallRequest {
            executor,
            tool,
            dependency,
        };
        match attempt_repository_install(&request, stderr, context, repo) {
            RepositoryInstall::Installed => return Ok(InstallOutcome::RepositoryRelease),
            RepositoryInstall::FallBackToCargo => {}
            RepositoryInstall::FallBackToCargoInstall => {
                cargo_install_plan = cargo_install_plan.skip_binstall();
            }
        }
    }

    install_tool_with_cargo(executor, cargo_install_plan, stderr, context)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstallMode {
    Binstall,
    CargoInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstallOutcome {
    RepositoryRelease,
    CargoBinstall,
    CargoInstall,
}

#[derive(Clone, Copy)]
struct CargoInstallPlan<'a> {
    tool: &'a DependencyTool,
    version: Option<&'a str>,
    skip_binstall: bool,
}

impl<'a> CargoInstallPlan<'a> {
    const fn new(tool: &'a DependencyTool) -> Self {
        Self {
            tool,
            version: None,
            skip_binstall: false,
        }
    }

    const fn with_version(self, version: &'a str) -> Self {
        Self {
            version: Some(version),
            ..self
        }
    }

    const fn skip_binstall(self) -> Self {
        Self {
            skip_binstall: true,
            ..self
        }
    }
}

fn install_tool_with_cargo(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<InstallOutcome> {
    // Always try binstall first if available; fall back to cargo install on failure
    if context.cargo_fallback_mode == InstallMode::Binstall && !cargo_install_plan.skip_binstall {
        if try_binstall(executor, cargo_install_plan, stderr, context.quiet)? {
            return Ok(InstallOutcome::CargoBinstall);
        }
        write_message(
            stderr,
            context.quiet,
            &format!(
                "cargo binstall failed for {}; falling back to cargo install.",
                cargo_install_plan.tool.package
            ),
        );
    }

    run_cargo_install(executor, cargo_install_plan, stderr, context.quiet)
}

fn try_binstall(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
    stderr: &mut dyn Write,
    quiet: bool,
) -> Result<bool> {
    let mut args = vec!["binstall", "-y"];
    if let Some(version) = cargo_install_plan.version {
        args.extend(["--version", version]);
    }
    args.push(cargo_install_plan.tool.package);
    let output = executor.run("cargo", &args)?;

    if !output.status.success() {
        return Ok(false);
    }

    if !is_tool_installed(executor, cargo_install_plan.tool) {
        return Ok(false);
    }

    write_message(
        stderr,
        quiet,
        &format!(
            "Installed {} with cargo binstall.",
            cargo_install_plan.tool.package
        ),
    );
    Ok(true)
}

fn run_cargo_install(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
    stderr: &mut dyn Write,
    quiet: bool,
) -> Result<InstallOutcome> {
    let mut args = vec!["install"];
    let success_message = cargo_install_plan.version.map_or_else(
        || {
            format!(
                "Installed {} with cargo install.",
                cargo_install_plan.tool.package
            )
        },
        |version| {
            args.extend(["--locked", "--version", version]);
            format!(
                "Installed {} from source with cargo install.",
                cargo_install_plan.tool.package
            )
        },
    );
    args.push(cargo_install_plan.tool.package);

    let output = executor.run("cargo", &args)?;
    if !output.status.success() {
        let message = command_error_message(&output);
        return Err(InstallerError::DependencyInstall {
            tool: cargo_install_plan.tool.package,
            message,
        });
    }

    if !is_tool_installed(executor, cargo_install_plan.tool) {
        return Err(InstallerError::DependencyInstall {
            tool: cargo_install_plan.tool.package,
            message: format!(
                "cargo install reported success, but {} is still unavailable",
                cargo_install_plan.tool.package
            ),
        });
    }

    write_message(stderr, quiet, &success_message);
    Ok(InstallOutcome::CargoInstall)
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

pub(super) fn write_message(stderr: &mut dyn Write, quiet: bool, message: &str) {
    if quiet {
        return;
    }
    // Progress output is best effort; a failed stderr write must not
    // abort the installation.
    drop(writeln!(stderr, "{message}"));
}

pub(super) fn command_succeeds(executor: &dyn CommandExecutor, cmd: &str, args: &[&str]) -> bool {
    executor
        .run(cmd, args)
        .is_ok_and(|output| output.status.success())
}

fn should_refresh_companions(outcome: InstallOutcome, status: DylintToolStatus) -> bool {
    outcome != InstallOutcome::RepositoryRelease && !status.dylint_link
}

fn update_status_after_install(
    status: &mut DylintToolStatus,
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    outcome: InstallOutcome,
) {
    if tool == &CARGO_DYLINT_TOOL {
        status.cargo_dylint = true;

        if should_refresh_companions(outcome, *status) {
            // Installing cargo-dylint locally can also provide dylint-link.
            status.dylint_link = is_tool_installed(executor, &DYLINT_LINK_TOOL);
        }
    } else if tool == &DYLINT_LINK_TOOL {
        status.dylint_link = true;
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
