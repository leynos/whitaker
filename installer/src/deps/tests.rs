//! Tests for Dylint tool dependency installation and fallback behaviour.

use super::*;
use crate::dependency_binaries::DependencyBinaryInstallError;
use crate::dirs::BaseDirs;
use crate::test_utils::{ExpectedCall, StubExecutor, failure_output, success_output};
use std::path::PathBuf;

struct StubDirs {
    bin_dir: Option<PathBuf>,
}

impl BaseDirs for StubDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        self.bin_dir.clone()
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        None
    }
}

struct StubRepositoryInstaller {
    result: std::result::Result<PathBuf, DependencyBinaryInstallError>,
}

impl DependencyBinaryInstaller for StubRepositoryInstaller {
    fn install(
        &self,
        _dependency: &crate::dependency_binaries::DependencyBinary,
        _target: &TargetTriple,
        _dirs: &dyn BaseDirs,
    ) -> std::result::Result<PathBuf, DependencyBinaryInstallError> {
        match &self.result {
            Ok(path) => Ok(path.clone()),
            Err(DependencyBinaryInstallError::Download { url, reason }) => {
                Err(DependencyBinaryInstallError::Download {
                    url: url.clone(),
                    reason: reason.clone(),
                })
            }
            Err(DependencyBinaryInstallError::Extraction { archive, reason }) => {
                Err(DependencyBinaryInstallError::Extraction {
                    archive: archive.clone(),
                    reason: reason.clone(),
                })
            }
            Err(DependencyBinaryInstallError::MissingBinaryInArchive { binary }) => {
                Err(DependencyBinaryInstallError::MissingBinaryInArchive {
                    binary: binary.clone(),
                })
            }
            Err(DependencyBinaryInstallError::Install { binary, reason }) => {
                Err(DependencyBinaryInstallError::Install {
                    binary: binary.clone(),
                    reason: reason.clone(),
                })
            }
            Err(DependencyBinaryInstallError::MissingBinDir) => {
                Err(DependencyBinaryInstallError::MissingBinDir)
            }
            Err(DependencyBinaryInstallError::NonUtf8BinDir(path)) => {
                Err(DependencyBinaryInstallError::NonUtf8BinDir(path.clone()))
            }
            Err(DependencyBinaryInstallError::Io(error)) => Err(DependencyBinaryInstallError::Io(
                std::io::Error::new(error.kind(), error.to_string()),
            )),
        }
    }
}

fn binstall_version_check(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "--version"],
        result,
    }
}

fn binstall_install(tool: &'static str, result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "-y", tool],
        result,
    }
}

fn cargo_install(tool: &'static str, result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["install", tool],
        result,
    }
}

fn cargo_dylint_check(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result,
    }
}

fn dylint_link_check(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "dylint-link",
        args: vec!["--version"],
        result,
    }
}

fn install_options<'a>(
    repository_installer: &'a dyn DependencyBinaryInstaller,
    quiet: bool,
) -> DependencyInstallOptions<'a> {
    let dirs = StubDirs {
        bin_dir: Some(PathBuf::from("/tmp/bin")),
    };
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    DependencyInstallOptions {
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
        cargo_dylint_check(Ok(success_output())),
        dylint_link_check(Ok(success_output())),
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
    let repository_installer = StubRepositoryInstaller {
        result: Ok(PathBuf::from("/tmp/bin/cargo-dylint")),
    };
    let executor = StubExecutor::new(vec![
        binstall_version_check(Ok(success_output())),
        cargo_dylint_check(Ok(success_output())),
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
    let repository_installer = StubRepositoryInstaller {
        result: Err(DependencyBinaryInstallError::Download {
            url: "https://example.test/archive".to_owned(),
            reason: "not found".to_owned(),
        }),
    };
    let executor = StubExecutor::new(vec![
        binstall_version_check(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(success_output())),
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
    let repository_installer = StubRepositoryInstaller {
        result: Err(DependencyBinaryInstallError::MissingBinDir),
    };
    let executor = StubExecutor::new(vec![
        binstall_version_check(Ok(failure_output("missing binstall"))),
        cargo_install("cargo-dylint", Ok(success_output())),
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
    let repository_installer = StubRepositoryInstaller {
        result: Ok(PathBuf::from("/tmp/bin/cargo-dylint")),
    };
    let executor = StubExecutor::new(vec![
        binstall_version_check(Ok(success_output())),
        cargo_dylint_check(Ok(failure_output("still missing"))),
        binstall_install("cargo-dylint", Ok(success_output())),
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
    let repository_installer = StubRepositoryInstaller {
        result: Err(DependencyBinaryInstallError::MissingBinDir),
    };
    let executor = StubExecutor::new(vec![
        binstall_version_check(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(failure_output("binstall failed"))),
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
            assert_eq!(message, "binstall failed");
        }
        other => panic!("unexpected error: {other}"),
    }
    executor.assert_finished();
}
