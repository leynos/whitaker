//! Install orchestration types and helpers for dependency tools.

use std::{io::Write, process::Output};

use super::{
    CARGO_DYLINT_TOOL,
    CommandExecutor,
    DEPENDENCY_TOOLS,
    DYLINT_LINK_TOOL,
    DependencyTool,
    DylintToolStatus,
    install_repository::{
        RepositoryInstall,
        RepositoryInstallRequest,
        attempt_repository_install,
        resolve_dependency_binary,
    },
    is_binstall_available,
    is_tool_installed,
};
use crate::{
    dependency_binaries::DependencyBinaryInstaller,
    error::{InstallerError, Result},
    installer_packaging::TargetTriple,
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
    /// Refuse `cargo install` when the published archive is absent.
    ///
    /// The Cargo fallback succeeds, so a run that took it installed a tool
    /// built from unpinned sources and reported success. That is the outcome
    /// that produced a green build with the wrong Dylint tooling.
    pub(super) no_source_fallback: bool,
}

/// Install each missing dependency and refresh availability after installation.
///
/// # Errors
///
/// Returns an error if installation, verification, or a follow-up availability
/// query fails.
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
        update_status_after_install(&mut remaining_status, executor, tool, outcome)?;
        write_message(
            stderr,
            context.quiet,
            install_success_message(tool, outcome, context.repo.is_some()),
        );
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

/// Select Cargo Binstall when available, otherwise use `cargo install`.
///
/// # Errors
///
/// Returns an error if the Binstall availability query fails.
pub(super) fn cargo_fallback_mode(executor: &dyn CommandExecutor) -> Result<InstallMode> {
    Ok(if is_binstall_available(executor)? {
        InstallMode::Binstall
    } else {
        InstallMode::CargoInstall
    })
}

pub(super) fn should_install_tool(status: &DylintToolStatus, tool: &DependencyTool) -> bool {
    if tool == &CARGO_DYLINT_TOOL {
        !status.cargo_dylint
    } else if tool == &DYLINT_LINK_TOOL {
        !status.dylint_link
    } else {
        false
    }
}

/// Install one dependency, preferring its repository release when configured.
///
/// # Errors
///
/// Returns an error if dependency metadata, installation, source policy, or
/// post-install verification fails.
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
        match attempt_repository_install(&request, repo)? {
            RepositoryInstall::Installed => return Ok(InstallOutcome::RepositoryRelease),
            RepositoryInstall::FallBack {
                reason,
                skip_binstall,
            } => {
                if skip_binstall {
                    cargo_install_plan = cargo_install_plan.skip_binstall();
                }
                refuse_or_announce_fallback(tool, &reason, stderr, context)?;
            }
        }
    }

    // Reached when no repository install was attempted at all, because the
    // base directories could not be resolved. The refusals inside the failure
    // arms never run on this path, so without this a run that forbids a source
    // build would take one anyway, quietly.
    if context.no_source_fallback {
        return Err(InstallerError::SourceFallbackForbidden {
            artefact: format!("a published {} archive", tool.package),
            reason: "no repository install was attempted, because the base directories could not \
                     be resolved"
                .to_owned(),
        });
    }

    install_tool_with_cargo(executor, cargo_install_plan, stderr, context)
}

/// Announce the Cargo fallback, or refuse it and say why it was not taken.
///
/// The decision belongs in each failure arm rather than after them. Deciding
/// afterwards printed "falling back to Cargo" and then refused to do it, which
/// tells an operator the opposite of what happened, and it discarded the
/// repository's own reason in favour of a generic one.
fn refuse_or_announce_fallback(
    tool: &DependencyTool,
    reason: &str,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<()> {
    if context.no_source_fallback {
        return Err(InstallerError::SourceFallbackForbidden {
            artefact: format!("a published {} archive", tool.package),
            reason: format!("the repository install {reason}"),
        });
    }
    write_message(
        stderr,
        context.quiet,
        format!(
            "Repository install for {} {reason}. Falling back to Cargo.",
            tool.package
        ),
    );
    Ok(())
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
    fn new(tool: &'a DependencyTool) -> Self {
        Self {
            tool,
            version: None,
            skip_binstall: false,
        }
    }

    fn with_version(self, version: &'a str) -> Self {
        Self {
            version: Some(version),
            ..self
        }
    }

    fn skip_binstall(self) -> Self {
        Self {
            skip_binstall: true,
            ..self
        }
    }
}

/// Install a dependency through Cargo, trying Binstall before source install.
///
/// # Errors
///
/// Returns an error if a Cargo command cannot run or source installation fails.
fn install_tool_with_cargo(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
) -> Result<InstallOutcome> {
    // Always try binstall first if available; fall back to cargo install on failure
    if context.cargo_fallback_mode == InstallMode::Binstall && !cargo_install_plan.skip_binstall {
        if try_binstall(executor, cargo_install_plan)? {
            return Ok(InstallOutcome::CargoBinstall);
        }
        write_message(
            stderr,
            context.quiet,
            format!(
                "cargo binstall failed for {}; falling back to cargo install.",
                cargo_install_plan.tool.package
            ),
        );
    }

    run_cargo_install(executor, cargo_install_plan)
}

/// Install through Cargo Binstall and verify the resulting executable.
fn try_binstall(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
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

    if !is_tool_installed(executor, cargo_install_plan.tool)? {
        return Ok(false);
    }

    Ok(true)
}

/// Install through Cargo and verify the resulting executable.
fn run_cargo_install(
    executor: &dyn CommandExecutor,
    cargo_install_plan: CargoInstallPlan<'_>,
) -> Result<InstallOutcome> {
    let mut args = vec!["install"];
    if let Some(version) = cargo_install_plan.version {
        args.extend(["--locked", "--version", version]);
    }
    args.push(cargo_install_plan.tool.package);

    let output = executor.run("cargo", &args)?;
    if !output.status.success() {
        let message = command_error_message(&output);
        return Err(InstallerError::DependencyInstall {
            tool: cargo_install_plan.tool.package,
            message,
        });
    }

    if !is_tool_installed(executor, cargo_install_plan.tool)? {
        return Err(InstallerError::DependencyInstall {
            tool: cargo_install_plan.tool.package,
            message: format!(
                "cargo install reported success, but {} is still unavailable",
                cargo_install_plan.tool.package
            ),
        });
    }

    Ok(InstallOutcome::CargoInstall)
}

/// Describe a completed install after its follow-up status checks succeed.
fn install_success_message(
    tool: &DependencyTool,
    outcome: InstallOutcome,
    has_pinned_version: bool,
) -> String {
    match outcome {
        InstallOutcome::RepositoryRelease => {
            format!("Installed {} from repository release.", tool.package)
        }
        InstallOutcome::CargoBinstall => {
            format!("Installed {} with cargo binstall.", tool.package)
        }
        InstallOutcome::CargoInstall if has_pinned_version => {
            format!("Installed {} from source with cargo install.", tool.package)
        }
        InstallOutcome::CargoInstall => {
            format!("Installed {} with cargo install.", tool.package)
        }
    }
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

/// Run a command and report whether it exits successfully.
///
/// # Errors
///
/// Returns an error if the command cannot be started or completed.
pub(super) fn command_succeeds(
    executor: &dyn CommandExecutor,
    cmd: &str,
    args: &[&str],
) -> Result<bool> {
    Ok(executor.run(cmd, args)?.status.success())
}

fn should_refresh_companions(outcome: InstallOutcome, status: &DylintToolStatus) -> bool {
    outcome != InstallOutcome::RepositoryRelease && !status.dylint_link
}

/// Record an installed tool and recheck companion binaries when needed.
///
/// # Errors
///
/// Returns an error if checking for the companion `dylint-link` binary fails.
fn update_status_after_install(
    status: &mut DylintToolStatus,
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    outcome: InstallOutcome,
) -> Result<()> {
    if tool == &CARGO_DYLINT_TOOL {
        status.cargo_dylint = true;

        if should_refresh_companions(outcome, status) {
            // Installing cargo-dylint locally can also provide dylint-link.
            status.dylint_link = is_tool_installed(executor, &DYLINT_LINK_TOOL)?;
        }
    } else if tool == &DYLINT_LINK_TOOL {
        status.dylint_link = true;
    }
    Ok(())
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
