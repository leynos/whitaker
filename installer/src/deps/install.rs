//! Install orchestration types and helpers for dependency tools.

use std::io::Write;

use super::{
    CARGO_DYLINT_TOOL,
    CommandExecutor,
    DEPENDENCY_TOOLS,
    DYLINT_LINK_TOOL,
    DependencyTool,
    DylintToolStatus,
    install_cargo::{
        CargoInstallPlan,
        InstallMode,
        InstallOutcome,
        install_success_message,
        install_tool_with_cargo,
        update_status_after_install,
        write_message,
    },
    install_repository::{
        RepositoryInstall,
        RepositoryInstallRequest,
        attempt_repository_install,
        resolve_dependency_binary,
    },
    is_binstall_available,
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
        (Some(base_dirs), Some(repo_installer), Some(host_target)) => {
            Some(RepositoryInstallContext {
                dirs: base_dirs,
                installer: repo_installer,
                target: host_target,
            })
        }
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

pub(super) fn should_install_tool(status: DylintToolStatus, tool: &DependencyTool) -> bool {
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
