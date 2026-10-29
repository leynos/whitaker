//! Compile-fail guard for consumers that do not handle dependency-check errors.

use whitaker_installer::error::InstallerError;

/// Match every pre-dependency-check error variant exhaustively.
fn legacy_error_kind(error: &InstallerError) -> &'static str {
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
        InstallerError::WrapperGeneration(_) => "wrapper",
        InstallerError::ClippySnapshot(_) => "Clippy snapshot",
        InstallerError::Bootstrap(_) => "bootstrap",
        InstallerError::ScanFailed { .. } => "scan",
        InstallerError::WriteFailed { .. } => "write",
    }
}

/// Keep the legacy exhaustive match reachable during compilation.
fn main() {
    let _ = legacy_error_kind;
}
