//! Tests for Dylint tool dependency installation and fallback behaviour.

use super::*;
use crate::dependency_binaries::{DependencyBinaryInstallError, MockDependencyBinaryInstaller};
use crate::installer_packaging::TargetTriple;
use crate::test_utils::dependency_binary_helpers::{
    binstall_install, binstall_version_check_with_result, cargo_dylint_check_with_result,
    cargo_install, dylint_link_check_with_result,
};
use crate::test_utils::{ExpectedCall, StubDirs, StubExecutor, failure_output, success_output};
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
        quiet,
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
fn check_dylint_tools_reports_installed_tools() {
    let executor = StubExecutor::new(vec![
        cargo_dylint_check_with_result(Ok(success_output())),
        dylint_link_check_with_result(Ok(success_output())),
    ]);

    let status = check_dylint_tools(&executor);

    assert_eq!(
        status,
        DylintToolStatus {
            cargo_dylint: true,
            dylint_link: true,
        }
    );
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_uses_repository_release_first() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        cargo_dylint_check_with_result(Ok(success_output())),
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
        binstall_install("cargo-dylint", Ok(success_output())),
        cargo_dylint_check_with_result(Ok(success_output())),
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
        cargo_install("cargo-dylint", Ok(success_output())),
        cargo_dylint_check_with_result(Ok(success_output())),
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
    assert!(output.contains("Installed cargo-dylint with cargo install."));
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_when_repository_verification_fails() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        cargo_dylint_check_with_result(Ok(failure_output("still missing"))),
        binstall_install("cargo-dylint", Ok(success_output())),
        cargo_dylint_check_with_result(Ok(success_output())),
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
        binstall_install("cargo-dylint", Ok(failure_output("binstall failed"))),
        cargo_install("cargo-dylint", Ok(failure_output("cargo install failed"))),
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
            url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0.tgz".to_owned(),
        })
    });
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "4.1.0", "cargo-dylint"],
            result: Ok(success_output()),
        },
        cargo_dylint_check_with_result(Ok(success_output())),
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
    assert!(output.contains("Building from source with Cargo."));
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
                url: "https://example.test/cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0.tgz"
                    .to_owned(),
            })
        });
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "4.1.0", "cargo-dylint"],
            result: Ok(success_output()),
        },
        cargo_dylint_check_with_result(Ok(success_output())),
        dylint_link_check_with_result(Ok(success_output())),
    ]);
    let mut stderr = Vec::new();

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

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
    assert!(!output.contains("Installed dylint-link"));
    executor.assert_finished();
}
