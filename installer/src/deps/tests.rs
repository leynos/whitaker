//! Tests for Dylint tool dependency installation and fallback behaviour.

use super::*;
use crate::dependency_binaries::{DependencyBinaryInstallError, MockDependencyBinaryInstaller};
use crate::installer_packaging::TargetTriple;
use crate::test_utils::dependency_binary_helpers::{
    binstall_install, binstall_version_check_with_result, cargo_dylint_check,
    cargo_dylint_check_with_result, dylint_link_install_list_check, with_fake_binary_on_path,
};
use crate::test_utils::{
    ExpectedCall, StubDirs, StubExecutor, failure_output, stdout_output, success_output,
};
use std::path::PathBuf;

fn install_options<'a>(
    repository_installer: &'a dyn DependencyBinaryInstaller,
    quiet: bool,
) -> DependencyInstallOptions<'a> {
    let dirs = StubDirs {
        bin_dir: Some(PathBuf::from("/tmp/bin")),
    };
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    DependencyInstallOptions {
        // Intentional leak in tests to extend lifetime for trait object; acceptable here.
        dirs: Box::leak(Box::new(dirs)),
        repository_installer,
        target: Some(target),
        policy: SourcePolicy {
            quiet,
            no_source_fallback: false,
        },
    }
}

#[test]
fn dylint_tool_status_all_installed_when_both_present() {
    assert!(
        DylintToolStatus {
            cargo_dylint: true,
            dylint_link: true,
        }
        .all_installed()
    );
}

#[test]
fn install_dylint_tools_uses_repository_release_first() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
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
    .expect("repository install should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed cargo-dylint from repository release."));
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_to_binstall_when_repository_unavailable() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().returning(|_, _, _| {
        Err(DependencyBinaryInstallError::Download {
            url: "https://example.test/archive".to_owned(),
            reason: "not found".to_owned(),
        })
    });
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
    .expect("cargo binstall fallback should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Repository install for cargo-dylint unavailable"));
    assert!(output.contains("Installed cargo-dylint with cargo binstall."));
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_to_cargo_install_when_binstall_missing() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Err(DependencyBinaryInstallError::MissingBinDir));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(failure_output("missing binstall"))),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "6.0.1", "cargo-dylint"],
            result: Ok(success_output()),
        },
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
    .expect("cargo install fallback should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
    executor.assert_finished();
}

/// Exercise source fallback when repository verification reports a stale version.
#[test]
fn install_dylint_tools_falls_back_when_repository_verification_reports_a_stale_version() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        cargo_dylint_check_with_result(Ok(stdout_output("cargo-dylint 5.0.0\n"))),
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
    .expect("fallback after verification failure should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("failed verification"));
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_reports_total_failure_after_all_fallbacks() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Err(DependencyBinaryInstallError::MissingBinDir));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(failure_output("binstall failed")))
            .expect("cargo-dylint dependency version should resolve"),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "6.0.1", "cargo-dylint"],
            result: Ok(failure_output("cargo install failed")),
        },
    ]);
    let mut stderr = Vec::new();

    let error = install_dylint_tools_with_options(
        &executor,
        &DylintToolStatus {
            cargo_dylint: false,
            dylint_link: true,
        },
        &mut stderr,
        install_options(&repository_installer, false),
    )
    .expect_err("install should fail after all fallbacks");

    match error {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, "cargo-dylint");
            assert_eq!(message, "cargo install failed");
        }
        other => panic!("unexpected error: {other}"),
    }
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_builds_from_source_when_repository_asset_is_missing() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().returning(|_, _, _| {
        Err(DependencyBinaryInstallError::NotFound {
            url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz".to_owned(),
        })
    });
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "6.0.1", "cargo-dylint"],
            result: Ok(success_output()),
        },
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
    .expect("source build should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Falling back to Cargo."));
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_skips_dylint_link_when_cargo_dylint_source_build_installs_it() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .once()
        .returning(|_, _, _| {
            Err(DependencyBinaryInstallError::NotFound {
                url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz"
                    .to_owned(),
            })
        });
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "6.0.1", "cargo-dylint"],
            result: Ok(success_output()),
        },
        cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
        // Refreshing the companion dylint-link status consults Cargo's
        // installed-binary registry once the PATH probe passes.
        dylint_link_install_list_check().expect("dylint-link dependency version should resolve"),
    ]);
    let mut stderr = Vec::new();

    with_fake_binary_on_path("dylint-link", || {
        install_dylint_tools_with_options(
            &executor,
            &DylintToolStatus {
                cargo_dylint: false,
                dylint_link: false,
            },
            &mut stderr,
            install_options(&repository_installer, false),
        )
        .expect("cargo-dylint source build should satisfy both tools");
    })
    .expect("stage fake dylint-link on PATH");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
    assert!(!output.contains("Installed dylint-link"));
    executor.assert_finished();
}

#[test]
fn install_tool_errors_when_dependency_manifest_entry_is_missing() {
    let missing_tool = DependencyTool {
        package: "missing-tool",
        command: "missing-tool",
        args: &["--version"],
    };
    let executor = StubExecutor::new(vec![]);
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().never();
    let dirs = StubDirs {
        bin_dir: Some(PathBuf::from("/tmp/bin")),
    };
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let mut stderr = Vec::new();

    let error = install_tool(
        &executor,
        &missing_tool,
        &mut stderr,
        &InstallContext {
            repo: repository_install_context(
                Some(&dirs),
                Some(&repository_installer as &dyn DependencyBinaryInstaller),
                Some(&target),
            ),
            cargo_fallback_mode: InstallMode::Binstall,
            quiet: false,
            no_source_fallback: false,
        },
    )
    .expect_err("missing dependency manifest entry should be an install error");

    match error {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, "missing-tool");
            assert_eq!(
                message,
                "dependency manifest is missing an entry for missing-tool"
            );
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(stderr.is_empty());
    executor.assert_finished();
}

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
            url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz".to_owned(),
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

#[path = "tests_repository.rs"]
mod repository;
