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
    output::write_stderr_line,
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

/// Builds a dependency from source when the prebuilt route was unavailable.
///
/// `cargo binstall` is tried first, and by the time this runs it has either
/// failed or been declined. The pinned version is requested with `--locked`
/// so the dependency builds from the lock file its publisher shipped rather
/// than from a freshly resolved graph; without the pin, a transitive crate
/// declared `rust-version` can be pulled in that the host toolchain refuses.
///
/// A refusal is not silent: [`source_install_failure_message`] turns it into
/// the commands that resolve it, because Cargo's own wording reads as a
/// Whitaker defect.
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
        let message = source_install_failure_message(
            &output,
            cargo_install_plan.tool,
            cargo_install_plan.version,
        );
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

/// Explain a failed `cargo install` source build.
///
/// Cargo's own message names the crate and the `rustc` it wanted, but not what
/// the user can do about it, so a version refusal is recognized here and given
/// the commands that resolve it.
///
/// Without this, the fallback fails with a bare toolchain error that reads as a
/// Whitaker defect. It is not: `cargo-dylint` 6.3.0's shipped `Cargo.lock` pins
/// `cargo-util` 0.2.32, which declares `rust-version = "1.97"`, so a host whose
/// *active* toolchain is Rust 1.96 or older cannot build it from source.
///
/// The advice cannot be `--toolchain`: that flag is resolved in step 3 of the
/// install flow, after dependencies are ensured in step 1, so it does not reach
/// this command. The message therefore names the explicit `cargo +<channel>`
/// invocation and the manual-install escape hatch instead.
fn source_install_failure_message(
    output: &Output,
    tool: &DependencyTool,
    version: Option<&str>,
) -> String {
    let message = command_error_message(output);
    if !is_rust_version_refusal(&message) {
        return message;
    }

    let version_flag = version.map_or_else(String::new, |pinned_version| {
        format!("--locked --version {pinned_version} ")
    });
    format!(
        "{message}\n{} needs a newer Rust toolchain to build from source. Install one with \
         `rustup toolchain install stable`, then run `cargo +stable install {version_flag}{}` and \
         rerun the installer. Alternatively install prebuilt copies of {} manually and rerun the \
         installer with `--skip-deps`, which skips the availability check for every tool.",
        tool.package,
        tool.package,
        dependency_tool_list(),
    )
}

/// Render every tool `--skip-deps` stops checking as one prose list.
///
/// Generated from [`DEPENDENCY_TOOLS`] so that adding a third tool cannot
/// leave the advice naming only the one whose build failed.
fn dependency_tool_list() -> String {
    DEPENDENCY_TOOLS
        .iter()
        .map(|tool| format!("`{}`", tool.package))
        .collect::<Vec<_>>()
        .join(" and ")
}

/// Returns whether `message` is Cargo refusing a dependency's `rust-version`.
///
/// Cargo words this per crate, for example "package `cargo-util v0.2.32`
/// cannot be built because it requires rustc 1.97 or newer"; the stable
/// fragments are the "requires rustc" clause and the `--ignore-rust-version`
/// hint, which Cargo prints with it. Matching either keeps the check working if
/// only one of the two wordings survives a Cargo rewrite.
fn is_rust_version_refusal(message: &str) -> bool {
    message.contains("requires rustc") || message.contains("--ignore-rust-version")
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

pub(super) fn write_message(stderr: &mut dyn Write, quiet: bool, message: impl std::fmt::Display) {
    if !quiet {
        write_stderr_line(stderr, message);
    }
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

fn should_refresh_companions(outcome: InstallOutcome, status: DylintToolStatus) -> bool {
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

        if should_refresh_companions(outcome, *status) {
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
