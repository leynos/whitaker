//! Verify installer command-query errors are propagated through fallback paths.

use super::*;

/// Assert that a dependency command's spawn error remains visible to callers.
fn assert_spawn_error(error: InstallerError, expected_message: &str) {
    match error {
        InstallerError::Io(source) => assert_eq!(source.to_string(), expected_message),
        other => panic!("expected command spawn error, got {other}"),
    }
}

/// Assert that a failed dependency path did not report a successful install.
fn assert_no_install_success_message(stderr: &[u8]) {
    let stderr = String::from_utf8_lossy(stderr);
    assert!(
        !stderr.contains("Installed "),
        "failure path emitted a success message: {stderr}"
    );
}

/// Preserve errors while deciding whether Cargo Binstall is available.
#[test]
fn binstall_availability_query_errors_are_propagated() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer.expect_install().never();
    let executor = StubExecutor::new(vec![binstall_version_check_with_result(Err(
        InstallerError::Io(std::io::Error::other("binstall lookup failed")),
    ))]);

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
    .expect_err("failed binstall lookup must stop installation");

    assert_spawn_error(error, "binstall lookup failed");
    assert_no_install_success_message(&stderr);
    executor.assert_finished();
}

/// Preserve errors from checking an installation reported by Cargo Binstall.
#[test]
fn post_binstall_availability_query_errors_are_propagated() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Err(DependencyBinaryInstallError::MissingBinDir));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(success_output()))
            .expect("binstall install call should be configured"),
        cargo_dylint_check_with_result(Err(InstallerError::Io(std::io::Error::other(
            "post-binstall lookup failed",
        )))),
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
    .expect_err("failed post-binstall verification must stop installation");

    assert_spawn_error(error, "post-binstall lookup failed");
    assert_no_install_success_message(&stderr);
    executor.assert_finished();
}

/// Fall back to Cargo when Binstall succeeds without making the tool available.
#[test]
fn unverified_binstall_install_falls_back_to_cargo_install() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Err(DependencyBinaryInstallError::MissingBinDir));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(success_output())),
        binstall_install("cargo-dylint", Ok(success_output()))
            .expect("binstall install call should be configured"),
        cargo_dylint_check_with_result(Ok(stdout_output("cargo-dylint 5.0.0\n"))),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--locked", "--version", "6.0.1", "cargo-dylint"],
            result: Ok(success_output()),
        },
        cargo_dylint_check().expect("cargo-dylint verification should be configured"),
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
    .expect("cargo install should recover from an unverified binstall result");

    let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(output.contains("falling back to cargo install"));
    assert!(output.contains("Installed cargo-dylint from source with cargo install."));
    executor.assert_finished();
}

/// Preserve errors from verifying a successful `cargo install` command.
#[test]
fn post_cargo_install_availability_query_errors_are_propagated() {
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
        cargo_dylint_check_with_result(Err(InstallerError::Io(std::io::Error::other(
            "post-cargo-install lookup failed",
        )))),
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
    .expect_err("failed post-cargo-install verification must stop installation");

    assert_spawn_error(error, "post-cargo-install lookup failed");
    assert_no_install_success_message(&stderr);
    executor.assert_finished();
}

/// Preserve errors while verifying a repository-installed `cargo-dylint`.
#[test]
fn repository_release_availability_query_errors_are_propagated() {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    repository_installer
        .expect_install()
        .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
    let executor = StubExecutor::new(vec![
        binstall_version_check_with_result(Ok(failure_output("missing binstall"))),
        cargo_dylint_check_with_result(Err(InstallerError::Io(std::io::Error::other(
            "repository verification lookup failed",
        )))),
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
    .expect_err("failed repository verification must stop installation");

    assert_spawn_error(error, "repository verification lookup failed");
    assert_no_install_success_message(&stderr);
    executor.assert_finished();
}

/// Preserve errors while refreshing companion `dylint-link` status.
#[test]
fn companion_refresh_query_errors_are_propagated() {
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
        cargo_dylint_check().expect("cargo-dylint verification should be configured"),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--list"],
            result: Err(InstallerError::Io(std::io::Error::other(
                "companion refresh lookup failed",
            ))),
        },
    ]);

    let mut stderr = Vec::new();
    let error = with_fake_binary_on_path("dylint-link", || {
        install_dylint_tools_with_options(
            &executor,
            &DylintToolStatus {
                cargo_dylint: false,
                dylint_link: false,
            },
            &mut stderr,
            install_options(&repository_installer, false),
        )
    })
    .expect("fake companion should be staged on PATH")
    .expect_err("failed companion refresh must stop installation");

    assert_spawn_error(error, "companion refresh lookup failed");
    assert_no_install_success_message(&stderr);
    executor.assert_finished();
}
