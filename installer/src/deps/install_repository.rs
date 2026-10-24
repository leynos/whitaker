//! Repository-release installation path for dependency tools.
//!
//! This module resolves release metadata and verifies installed tools while
//! keeping repository-download orchestration out of the Cargo fallback path.

use super::{
    CommandExecutor, DYLINT_LINK_TOOL, DependencyTool, InstallerError, RepositoryInstallContext,
    Result, find_dependency_binary, is_tool_installed,
};
use crate::dependency_binaries::DependencyBinary;

/// Result of trying to install one dependency from a repository release.
pub(super) enum RepositoryInstall {
    /// The release installed successfully and passed verification.
    Installed,
    /// The caller should continue with Cargo; skip Binstall when no archive exists.
    FallBack { reason: String, skip_binstall: bool },
}

/// Resolve the manifest entry describing the release asset for a tool.
///
/// # Errors
///
/// Return an error if the dependency manifest cannot be read or parsed, or if
/// it has no entry for `tool.package`.
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

/// The collaborators needed to attempt a repository-release install.
pub(super) struct RepositoryInstallRequest<'a> {
    /// Executor used to check whether the installed binary runs.
    pub(super) executor: &'a dyn CommandExecutor,
    /// The dependency tool being installed.
    pub(super) tool: &'a DependencyTool,
    /// Release metadata identifying the artefact to fetch.
    pub(super) dependency: &'static DependencyBinary,
}

/// Try a repository release and report whether Cargo fallback is needed.
///
/// # Errors
///
/// Returns an error if the post-install verification command cannot run.
pub(super) fn attempt_repository_install(
    request: &RepositoryInstallRequest<'_>,
    repo: &RepositoryInstallContext<'_>,
) -> Result<RepositoryInstall> {
    match repo
        .installer
        .install(request.dependency, repo.target, repo.dirs)
    {
        Ok(_) if repository_install_verified(request.executor, request.tool)? => {
            Ok(RepositoryInstall::Installed)
        }
        Ok(_) => Ok(RepositoryInstall::FallBack {
            reason: "failed verification".to_owned(),
            skip_binstall: false,
        }),
        Err(error) => {
            let skip_binstall = error.is_not_found();
            Ok(RepositoryInstall::FallBack {
                reason: format!("unavailable: {error}"),
                skip_binstall,
            })
        }
    }
}

/// Verify a repository-release install of a dependency tool.
///
/// A verified release asset has passed the archive name, checksum, extraction,
/// and executable-permission checks. The generic health check remains useful
/// for cargo-dylint, while dylint-link is a linker wrapper and has no reliable
/// self-reporting command.
///
/// # Errors
///
/// Return an error if the generic installed-tool health check cannot complete.
fn repository_install_verified(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
) -> Result<bool> {
    if tool == &DYLINT_LINK_TOOL {
        return Ok(true);
    }
    is_tool_installed(executor, tool)
}
