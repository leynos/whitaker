//! Repository-release installation path for dependency tools.
//!
//! Whitaker publishes prebuilt `cargo-dylint` and `dylint-link` binaries as
//! release assets. This module resolves the manifest entry for a tool, drives
//! the release install, and reports which Cargo fallback (if any) remains
//! viable when the release cannot be used.

use std::io::Write;

use super::{
    CommandExecutor,
    DYLINT_LINK_TOOL,
    DependencyTool,
    InstallContext,
    InstallerError,
    RepositoryInstallContext,
    Result,
    find_dependency_binary,
    is_tool_installed,
    write_message,
};
use crate::dependency_binaries::DependencyBinary;

/// Outcome of attempting a repository-release install for one dependency tool.
pub(super) enum RepositoryInstall {
    /// The release was installed and verified; no Cargo fallback is needed.
    Installed,
    /// The release was unusable; fall back to the configured Cargo mode.
    FallBackToCargo,
    /// The release is absent upstream, so `cargo binstall` cannot help either.
    FallBackToCargoInstall,
}

/// Looks up the manifest entry describing the release asset for `tool`.
pub(super) fn resolve_dependency_binary(
    tool: &DependencyTool,
) -> Result<&'static DependencyBinary> {
    let entry = find_dependency_binary(tool.package).map_err(|error| {
        InstallerError::DependencyInstall {
            tool: tool.package,
            message: error.to_string(),
        }
    })?;

    entry.ok_or_else(|| InstallerError::DependencyInstall {
        tool: tool.package,
        message: format!(
            "dependency manifest is missing an entry for {}",
            tool.package
        ),
    })
}

/// The tool to install from a repository release, together with the
/// collaborators needed to perform and verify that install.
///
/// Grouped so [`attempt_repository_install`] stays within the workspace
/// argument budget; the three fields are only ever supplied together.
pub(super) struct RepositoryInstallRequest<'a> {
    /// Executor used to probe whether the installed binary runs.
    pub(super) executor: &'a dyn CommandExecutor,
    /// The dependency tool being installed.
    pub(super) tool: &'a DependencyTool,
    /// Release metadata identifying the artefact to fetch.
    pub(super) dependency: &'static DependencyBinary,
}

/// Installs the requested tool from a repository release, reporting whether
/// Cargo must still run and, if so, which fallback mode remains viable.
pub(super) fn attempt_repository_install(
    request: &RepositoryInstallRequest<'_>,
    stderr: &mut dyn Write,
    context: &InstallContext<'_>,
    repo: &RepositoryInstallContext<'_>,
) -> RepositoryInstall {
    let tool = request.tool;
    match repo
        .installer
        .install(request.dependency, repo.target, repo.dirs)
    {
        Ok(_) if repository_install_verified(request.executor, tool) => {
            write_message(
                stderr,
                context.quiet,
                &format!("Installed {} from repository release.", tool.package),
            );
            RepositoryInstall::Installed
        }
        Ok(_) => {
            write_message(
                stderr,
                context.quiet,
                &format!(
                    "Repository install for {} failed verification; falling back to Cargo.",
                    tool.package
                ),
            );
            RepositoryInstall::FallBackToCargo
        }
        Err(error) => {
            let not_found = error.is_not_found();
            write_message(
                stderr,
                context.quiet,
                &format!(
                    "Repository install for {} unavailable: {error}. Falling back to Cargo.",
                    tool.package
                ),
            );
            if not_found {
                RepositoryInstall::FallBackToCargoInstall
            } else {
                RepositoryInstall::FallBackToCargo
            }
        }
    }
}

/// Verify a repository-release install of `tool`.
///
/// The trust boundary for a repository install is established entirely by the
/// installer pipeline: the release asset name pins the package and version,
/// the `.sha256` sidecar establishes integrity, extraction confirms the
/// expected archive member, and the permission step establishes launch
/// eligibility. A successful install is therefore sufficient evidence on its
/// own.
///
/// `dylint-link` is additionally never executed as a health check. It is a
/// linker wrapper that forwards its entire argument list to the underlying
/// linker, so it has no reliable self-reporting subcommand: `--version` exits
/// early and `--help` depends on a usable linker and toolchain in the ambient
/// environment. Probing it rejects valid, verified artefacts and forces a
/// source build that cannot succeed on toolchains older than the crate's
/// rustc floor.
///
/// `cargo-dylint` keeps the generic check because it reports its own version
/// and must additionally be discoverable by Cargo as a subcommand.
fn repository_install_verified(executor: &dyn CommandExecutor, tool: &DependencyTool) -> bool {
    if tool == &DYLINT_LINK_TOOL {
        return true;
    }
    is_tool_installed(executor, tool)
}
