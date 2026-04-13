//! Tests for Dylint tool dependency installation and fallback behaviour.

use super::*;
use crate::dependency_binaries::{DependencyBinaryInstallError, MockDependencyBinaryInstaller};
use crate::installer_packaging::TargetTriple;
use crate::test_support::env_test_guard;
use crate::test_utils::dependency_binary_helpers::{
    binstall_install, binstall_version_check_with_result, cargo_dylint_check_with_result,
    with_fake_binary_on_path, with_fake_path, write_fake_binary,
};
use crate::test_utils::{ExpectedCall, StubDirs, StubExecutor, failure_output, success_output};
use std::path::PathBuf;
use temp_env::with_vars_unset;

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
    with_fake_binary_on_path("dylint-link", || {
        let executor =
            StubExecutor::new(vec![cargo_dylint_check_with_result(Ok(success_output()))]);

        let status = check_dylint_tools(&executor);

        assert_eq!(
            status,
            DylintToolStatus {
                cargo_dylint: true,
                dylint_link: true,
            }
        );
        executor.assert_finished();
    });
}

#[test]
fn is_binary_on_path_returns_false_when_path_is_unset() {
    with_vars_unset(["PATH"], || {
        assert!(!is_binary_on_path("dylint-link"));
    });
}

#[test]
fn is_binary_on_path_returns_false_when_path_is_empty() {
    let _guard = env_test_guard();
    temp_env::with_var("PATH", Some(""), || {
        assert!(!is_binary_on_path("dylint-link"));
    });
}

#[test]
fn is_binary_on_path_returns_false_when_binary_is_missing_from_all_directories() {
    with_fake_path(
        |_| {},
        || {
            assert!(!is_binary_on_path("dylint-link"));
        },
    );
}

#[test]
fn is_binary_on_path_checks_multiple_directories() {
    with_fake_path(
        |directories| write_fake_binary(&directories[1].join("dylint-link"), true),
        || {
            assert!(is_binary_on_path("dylint-link"));
        },
    );
}

#[cfg(unix)]
#[test]
fn is_executable_file_rejects_non_executable_files() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let binary_path = temp_dir.path().join("dylint-link");
    write_fake_binary(&binary_path, false);

    assert!(!is_executable_file(&binary_path));
}

#[cfg(unix)]
#[test]
fn is_executable_file_accepts_executable_files() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let binary_path = temp_dir.path().join("dylint-link");
    write_fake_binary(&binary_path, true);

    assert!(is_executable_file(&binary_path));
}

#[cfg(windows)]
#[test]
fn is_binary_on_path_accepts_windows_executable_suffix() {
    with_fake_path(
        |directories| write_fake_binary(&directories[0].join("dylint-link.exe"), true),
        || {
            assert!(is_binary_on_path("dylint-link"));
        },
    );
}

#[cfg(windows)]
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() {
    let _guard = env_test_guard();
    temp_env::with_var("PATHEXT", Some(".CMD;.BAT"), || {
        with_fake_path(
            |directories| write_fake_binary(&directories[0].join("dylint-link.cmd"), true),
            || {
                let executor =
                    StubExecutor::new(vec![cargo_dylint_check_with_result(Ok(success_output()))]);

                let status = check_dylint_tools(&executor);

                assert_eq!(
                    status,
                    DylintToolStatus {
                        cargo_dylint: true,
                        dylint_link: true,
                    }
                );
                executor.assert_finished();
            },
        );
    });
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
    .expect("cargo install fallback should succeed");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
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
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "4.1.0", "cargo-dylint"],
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
    });

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
