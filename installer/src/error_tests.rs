//! Unit tests for installer error formatting and cloning.

use super::*;

#[test]
fn toolchain_not_installed_suggests_install_command() {
    let err = InstallerError::ToolchainNotInstalled {
        toolchain: "nightly-2026-05-28".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("rustup toolchain install"));
    assert!(msg.contains("nightly-2026-05-28"));
}

#[test]
fn toolchain_install_failed_includes_toolchain_and_message() {
    let err = InstallerError::ToolchainInstallFailed {
        toolchain: "nightly-2026-05-28".to_owned(),
        message: "network error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("nightly-2026-05-28"));
    assert!(msg.contains("network error"));
}

#[test]
fn toolchain_component_install_failed_includes_components() {
    let err = InstallerError::ToolchainComponentInstallFailed {
        toolchain: "nightly-2026-05-28".to_owned(),
        components: "rust-src, rustc-dev".to_owned(),
        message: "component error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("nightly-2026-05-28"));
    assert!(msg.contains("rust-src, rustc-dev"));
    assert!(msg.contains("component error"));
}

#[test]
fn build_failed_includes_crate_name() {
    let err = InstallerError::BuildFailed {
        crate_name: CrateName::from("module_max_lines"),
        reason: "compilation error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("module_max_lines"));
    assert!(msg.contains("compilation error"));
}

#[test]
fn git_error_includes_operation_and_message() {
    let err = InstallerError::Git {
        operation: "clone",
        message: "network error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("clone"));
    assert!(msg.contains("network error"));
}

#[test]
fn dependency_install_error_includes_tool_name() {
    let err = InstallerError::DependencyInstall {
        tool: "cargo-dylint",
        message: "network error".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("cargo-dylint"));
    assert!(msg.contains("network error"));
}

#[test]
fn wrapper_generation_error_includes_message() {
    let err = InstallerError::WrapperGeneration("permission denied".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
}

#[test]
fn scan_failed_includes_reason() {
    let source = std::io::Error::other("directory not found");
    let err = InstallerError::ScanFailed { source };
    let msg = err.to_string();
    assert!(msg.contains("scan"));
    // Verify the source error is preserved via the Error trait
    let source_err = std::error::Error::source(&err);
    assert!(source_err.is_some());
}

#[test]
fn write_failed_includes_reason() {
    let source = std::io::Error::other("permission denied");
    let err = InstallerError::WriteFailed { source };
    let msg = err.to_string();
    assert!(msg.contains("write"));
    // Verify the source error is preserved via the Error trait
    let source_err = std::error::Error::source(&err);
    assert!(source_err.is_some());
}
