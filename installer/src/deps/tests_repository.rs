//! Tests for repository-release installation of the Dylint dependency tools.

use std::path::PathBuf;

use super::{
    DependencyBinaryInstaller, DependencyTool, DylintToolStatus,
    install::{InstallContext, InstallMode, install_tool, repository_install_context},
    install_dylint_tools_with_options, install_options,
};
use crate::dependency_binaries::{DependencyBinaryInstallError, MockDependencyBinaryInstaller};
use crate::error::InstallerError;
use crate::installer_packaging::TargetTriple;
use crate::test_utils::{
    StubDirs, StubExecutor,
    dependency_binary_helpers::{
        binstall_install, binstall_version_check_with_result, dylint_link_install_list_check,
        with_fake_binary_on_path,
    },
    success_output,
};

/// Writes a fake `dylint-link` into a temporary directory and returns the
/// directory guard alongside the binary path.
///
/// The fake is staged with a failing exit status so that any attempt to
/// execute it as a health check would be observable as a verification
/// failure.
fn staged_unrunnable_dylint_link() -> std::io::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = crate::test_utils::dependency_binary_helpers::path_binary_location(
        dir.path(),
        "dylint-link",
    );
    crate::test_utils::dependency_binary_helpers::write_fake_binary_with_status(&path, true, 1);
    Ok((dir, path))
}

#[test]
fn install_dylint_tools_accepts_repository_dylint_link_without_executing_it() {
    // `dylint-link` forwards its argument list to the underlying linker and
    // has no reliable self-reporting subcommand, so a successful repository
    // install is accepted on the strength of the download, checksum,
    // extraction, and permission steps alone. Staging a fake that exits
    // non-zero proves the binary is never executed as a health check: any
    // such probe would reject it and fall back to Cargo.
    let (_dir, binary_path) = staged_unrunnable_dylint_link().expect("stage fake dylint-link");
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .once()
        .returning(move |_, _, _| Ok(binary_path.clone()));
    let executor = StubExecutor::new(vec![binstall_version_check_with_result(Ok(
        success_output(),
    ))]);
    let mut stderr = Vec::new();

    install_dylint_tools_with_options(
        &executor,
        &DylintToolStatus {
            cargo_dylint: true,
            dylint_link: false,
        },
        &mut stderr,
        install_options(&repository_installer, false),
    )
    .expect("repository install should satisfy dylint-link");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Installed dylint-link from repository release."));
    assert!(!output.contains("failed verification"));
    // No Cargo command beyond the binstall-availability probe may run: a
    // source build of dylint-link cannot succeed on toolchains below the
    // crate's rustc floor.
    executor.assert_finished();
}

#[test]
fn install_dylint_tools_falls_back_when_repository_dylint_link_install_fails() {
    // Genuine repository failures — missing asset, checksum mismatch,
    // extraction failure, or an unwritable executable — must still fall back
    // to Cargo.
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().returning(|_, _, _| {
        Err(DependencyBinaryInstallError::Install {
            binary: "dylint-link".to_owned(),
            reason: "checksum mismatch".to_owned(),
        })
    });
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        binstall_install("dylint-link", Ok(success_output())),
        // The post-binstall check resolves the PATH binary and then confirms
        // the version against Cargo's installed-binary registry.
        dylint_link_install_list_check(),
    ]);
    let mut stderr = Vec::new();

    with_fake_binary_on_path("dylint-link", || {
        install_dylint_tools_with_options(
            &executor,
            &DylintToolStatus {
                cargo_dylint: true,
                dylint_link: false,
            },
            &mut stderr,
            install_options(&repository_installer, false),
        )
        .expect("binstall fallback should succeed");
    });

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("Repository install for dylint-link unavailable"));
    assert!(output.contains("Installed dylint-link with cargo binstall."));
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
