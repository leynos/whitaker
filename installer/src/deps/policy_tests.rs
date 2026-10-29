//! Source-fallback policy tests for Dylint dependency installation.
//!
//! These cover the `SourcePolicy::no_source_fallback` guard: a run that
//! forbids a source build must refuse rather than fall back to `cargo
//! install`. Split from `tests.rs` to keep each file inside the repository's
//! 400-line module limit.

use super::*;
use crate::dependency_binaries::MockDependencyBinaryInstaller;
use crate::error::InstallerError;
use crate::test_utils::dependency_binary_helpers::{
    binstall_install, binstall_version_check_with_result, cargo_dylint_check,
};
use crate::test_utils::{StubExecutor, success_output};

/// Install options that refuse a source build, as CI sets them.
fn strict_install_options(
    repository_installer: &dyn DependencyBinaryInstaller,
) -> DependencyInstallOptions<'_> {
    let mut options = install_options(repository_installer, false);
    options.policy.no_source_fallback = true;
    options
}

/// A repository installer whose archive is absent, as during a republish.
fn installer_missing_its_archive() -> MockDependencyBinaryInstaller {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().returning(|_, _, _| {
        Err(DependencyBinaryInstallError::Download {
            url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v6.3.0.tgz".to_owned(),
            reason: "not found".to_owned(),
        })
    });
    repository_installer
}

#[test]
fn a_missing_dylint_archive_fails_under_the_no_source_fallback_policy() {
    // The executor expects nothing: refusing means no Cargo command runs at
    // all, which is the assertion that matters. A test that only checked the
    // error would pass while `cargo install` had already been attempted.
    let repository_installer = installer_missing_its_archive();
    let executor = StubExecutor::new(vec![binstall_version_check_with_result(Ok(
        success_output(),
    ))]);
    let mut stderr = Vec::new();

    let error = install_dylint_tools_with_options(
        &executor,
        &DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
        &mut stderr,
        strict_install_options(&repository_installer),
    )
    .expect_err("an absent archive must fail when a source build is forbidden");

    assert!(
        matches!(error, InstallerError::SourceFallbackForbidden { .. }),
        "expected a source-fallback refusal, got: {error}"
    );
    assert!(
        error.to_string().contains("cargo-dylint"),
        "the refusal must name the tool, got: {error}"
    );
}

#[test]
fn the_same_missing_archive_falls_back_without_the_policy() {
    // The pair is the point: the flag changes the outcome for one input, so
    // the default path is asserted on the same input rather than assumed.
    let repository_installer = installer_missing_its_archive();
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(success_output()))
            .expect("cargo-dylint dependency version should resolve"),
        cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
    ]);
    let mut stderr = Vec::new();

    install_dylint_tools_with_options(
        &executor,
        &DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
        &mut stderr,
        install_options(&repository_installer, false),
    )
    .expect("the default path should still fall back");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Repository install for cargo-dylint unavailable"));
    executor.assert_finished();
}

#[test]
fn an_unresolvable_repository_context_still_refuses_a_source_build() {
    // The hole this closes: with no base directories the repository install is
    // never attempted, so the refusals inside the failure arms never run and
    // `cargo install` was reached unguarded. A run that forbade a source build
    // would have taken one, and reported success.
    //
    // The executor expects nothing. Asserting only the error would pass while
    // Cargo had already been invoked, which is the failure itself.
    let executor = StubExecutor::new(vec![]);
    let mut stderr = Vec::new();

    let error = install_missing_tools(
        &executor,
        &DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
        &mut stderr,
        &InstallContext {
            repo: None,
            cargo_fallback_mode: InstallMode::CargoInstall,
            quiet: false,
            no_source_fallback: true,
        },
    )
    .expect_err("no repository context must not become a licence to build");

    assert!(
        matches!(error, InstallerError::SourceFallbackForbidden { .. }),
        "expected a source-fallback refusal, got: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("cargo-dylint"),
        "the refusal must name the tool, got: {message}"
    );
    assert!(
        message.contains("base") && message.contains("directories"),
        "the reason must say why no repository install was attempted, not \
         repeat the generic one, got: {message}"
    );
    executor.assert_finished();
}
