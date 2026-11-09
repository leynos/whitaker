//! Cargo fallback install path for dependency tools.
//!
//! Reached only after the repository release install has been declined or has
//! failed, so this module owns `cargo binstall` and `cargo install` and the
//! operator-facing messages that explain which of the two ran.

use std::{io::Write, process::Output};

use super::{
    CARGO_DYLINT_TOOL,
    CommandExecutor,
    DEPENDENCY_TOOLS,
    DYLINT_LINK_TOOL,
    DependencyTool,
    DylintToolStatus,
    install::InstallContext,
    is_tool_installed,
};
use crate::{
    error::{InstallerError, Result},
    output::write_stderr_line,
};

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
pub(super) struct CargoInstallPlan<'a> {
    tool: &'a DependencyTool,
    version: Option<&'a str>,
    skip_binstall: bool,
}

impl<'a> CargoInstallPlan<'a> {
    pub(super) const fn new(tool: &'a DependencyTool) -> Self {
        Self {
            tool,
            version: None,
            skip_binstall: false,
        }
    }

    pub(super) const fn with_version(self, version: &'a str) -> Self {
        Self {
            version: Some(version),
            ..self
        }
    }

    pub(super) const fn skip_binstall(self) -> Self {
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
pub(super) fn install_tool_with_cargo(
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
pub(super) fn run_cargo_install(
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
pub(super) fn install_success_message(
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
pub(super) fn source_install_failure_message(
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
pub(super) fn is_rust_version_refusal(message: &str) -> bool {
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

pub(super) fn should_refresh_companions(outcome: InstallOutcome, status: DylintToolStatus) -> bool {
    outcome != InstallOutcome::RepositoryRelease && !status.dylint_link
}

/// Record an installed tool and recheck companion binaries when needed.
///
/// # Errors
///
/// Returns an error if checking for the companion `dylint-link` binary fails.
pub(super) fn update_status_after_install(
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
