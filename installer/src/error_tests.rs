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

#[test]
fn conflicting_source_options_names_the_offending_option() {
    // The message is what a caller acts on, so it has to name both the option
    // that requires a source build and the two ways the rule can be enabled.
    let err = InstallerError::ConflictingSourceOptions {
        option: "--build-only".to_owned(),
    };

    let message = err.to_string();
    assert!(message.contains("--build-only"), "got: {message}");
    assert!(message.contains("--no-source-fallback"), "got: {message}");
    assert!(
        message.contains("WHITAKER_NO_SOURCE_FALLBACK"),
        "the environment route must be named too, got: {message}"
    );
}

#[test]
fn source_fallback_forbidden_carries_the_artefact_and_the_reason() {
    // Without the reason the operator learns only that something was absent,
    // and the download's own explanation is the part that says what to do.
    let err = InstallerError::SourceFallbackForbidden {
        artefact: "a published cargo-dylint archive".to_owned(),
        reason: "the repository install unavailable: not found".to_owned(),
    };

    let message = err.to_string();
    assert!(
        message.contains("a published cargo-dylint archive"),
        "got: {message}"
    );
    assert!(message.contains("not found"), "got: {message}");
}

#[test]
fn the_policy_errors_survive_a_clone() {
    // `InstallerError::clone` matches every variant by hand, so a new variant
    // that nobody added an arm for is a panic waiting for the first caller.
    for err in [
        InstallerError::ConflictingSourceOptions {
            option: "--experimental".to_owned(),
        },
        InstallerError::SourceFallbackForbidden {
            artefact: "a prebuilt lint library".to_owned(),
            reason: "repository asset not found".to_owned(),
        },
    ] {
        assert_eq!(err.clone().to_string(), err.to_string());
    }
}
