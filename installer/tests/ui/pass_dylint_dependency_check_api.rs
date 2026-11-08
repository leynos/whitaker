//! Compile-pass guard for the public installer dependency-check API.

use whitaker_installer::{
    deps::{check_dylint_tools, CommandExecutor, DylintToolStatus},
    error::{InstallerError, Result},
};

/// Return the dependency-query result through its public API.
fn check(executor: &dyn CommandExecutor) -> Result<DylintToolStatus> {
    check_dylint_tools(executor)
}

/// Match every public installer error, including dependency checks.
fn error_kind(error: &InstallerError) -> &'static str {
    match error {
        InstallerError::ConflictingSourceOptions { .. } => "source policy",
        InstallerError::SourceFallbackForbidden { .. } => "source fallback",
        InstallerError::ToolchainDetection { .. } => "toolchain detection",
        InstallerError::ToolchainFileNotFound { .. } => "toolchain file",
        InstallerError::InvalidToolchainFile { .. } => "toolchain file",
        InstallerError::ToolchainNotInstalled { .. } => "toolchain install",
        InstallerError::ToolchainInstallFailed { .. } => "toolchain install",
        InstallerError::ToolchainComponentInstallFailed { .. } => "toolchain component",
        InstallerError::BuildFailed { .. } => "build",
        InstallerError::StagingFailed { .. } => "staging",
        InstallerError::StubMismatch { .. } => "stub mismatch",
        InstallerError::TargetNotWritable { .. } => "target directory",
        InstallerError::LintCrateNotFound { .. } => "lint crate",
        InstallerError::ExperimentalLintRequiresFlag { .. } => "experimental lint",
        InstallerError::WorkspaceNotFound { .. } => "workspace",
        InstallerError::SuitePinInWorkspace { .. } => "suite pin",
        InstallerError::InvalidCargoToml { .. } => "manifest",
        InstallerError::Io(_) => "I/O",
        InstallerError::Git { .. } => "Git",
        InstallerError::DependencyInstall { .. } => "dependency install",
        InstallerError::DependencyCheck { .. } => "dependency check",
        InstallerError::WrapperGeneration(_) => "wrapper",
        InstallerError::ClippySnapshot(_) => "Clippy snapshot",
        InstallerError::Bootstrap(_) => "bootstrap",
        InstallerError::ScanFailed { .. } => "scan",
        InstallerError::WriteFailed { .. } => "write",
    }
}

/// Exercise the public result and exhaustive error match without a host command.
fn main() {
    let _ = check;
    let _ = error_kind;
}
