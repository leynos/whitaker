//! Tests for ensuring the Dylint dependency tools are installed.
//!
//! These exercise the installer boundary: which tool statuses trigger an
//! install, what progress output a run writes, and how a failed Cargo install
//! surfaces. The fixtures live in the parent module because the other CLI
//! tests share them.

use super::*;
use crate::{
    deps::install_dylint_tools_with_options,
    test_utils::{
        dependency_binary_helpers::{
            AlwaysNotFoundRepositoryInstaller,
            cargo_dylint_check,
            cargo_dylint_stale_version_check,
            dylint_link_install_list_check,
            with_fake_binary_on_path,
        },
        *,
    },
};

/// Skip installation when both required Dylint tools are already installed.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[rstest]
fn ensure_dylint_tools_skips_install_when_installed(
    test_base_dirs: TestBaseDirs,
) -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
        ]);
        let repository_installer = AlwaysNotFoundRepositoryInstaller;

        let mut stderr_output = Vec::new();
        let options = dependency_install_options(&test_base_dirs, &repository_installer, false)
            .expect("dependency install options should build");
        let result = ensure_dylint_tools_with_executor(
            &executor,
            options.policy,
            &mut stderr_output,
            |status, stderr| install_dylint_tools_with_options(&executor, status, stderr, options),
        );

        assert_check_succeeded(&result);
        assert_silent_skip(&stderr_output);
        executor.assert_finished();
    })?;
    Ok(())
}

/// Install missing Dylint tools and check the configured progress output.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[rstest]
#[case::logs_when_not_quiet(
    false,
    "Installing required Dylint tools...\n",
    "Dylint tools installed successfully.\n\n"
)]
#[case::quiet_suppresses_output(true, "", "")]
/// A missing `cargo-dylint` is installed at the pinned version, and the
/// progress messages respect `quiet`.
///
/// The stub executor asserts the exact argv, which is what pins the pin: the
/// install must carry `--locked --version 6.3.0` or the fallback would build
/// whatever version happens to be current.
fn ensure_dylint_tools_installs_missing_tools(
    test_base_dirs: TestBaseDirs,
    #[case] quiet: bool,
    #[case] expected_start: &str,
    #[case] expected_end: &str,
) -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_stale_version_check().expect("stale cargo-dylint version should resolve"),
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
            ExpectedCall {
                cmd: "cargo",
                args: vec!["binstall", "--version"],
                result: Ok(success_output()),
            },
            ExpectedCall {
                cmd: "cargo",
                args: vec!["install", "--locked", "--version", "6.3.0", "cargo-dylint"],
                result: Ok(success_output()),
            },
            cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
        ]);
        let repository_installer = AlwaysNotFoundRepositoryInstaller;

        let mut stderr_output = Vec::new();
        let options = dependency_install_options(&test_base_dirs, &repository_installer, quiet)
            .expect("dependency install options should build");
        let result = ensure_dylint_tools_with_executor(
            &executor,
            options.policy,
            &mut stderr_output,
            |status, stderr| install_dylint_tools_with_options(&executor, status, stderr, options),
        );

        assert_check_succeeded(&result);
        assert_install_progress(&stderr_output, quiet, expected_start, expected_end);
        executor.assert_finished();
    })?;
    Ok(())
}

/// Propagate a failure from the dependency installation command.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[rstest]
/// A failed install surfaces as a `DependencyInstall` naming the tool and
/// Cargo's own message, so the caller can report which step failed.
///
/// The stub stops after the failing call, so `assert_finished` also proves the
/// flow does not carry on and try to verify a tool it never installed.
fn ensure_dylint_tools_propagates_install_failures(
    test_base_dirs: TestBaseDirs,
) -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_stale_version_check().expect("stale cargo-dylint version should resolve"),
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
            ExpectedCall {
                cmd: "cargo",
                args: vec!["binstall", "--version"],
                result: Ok(success_output()),
            },
            ExpectedCall {
                cmd: "cargo",
                args: vec!["install", "--locked", "--version", "6.3.0", "cargo-dylint"],
                result: Ok(failure_output("cargo install failed")),
            },
        ]);
        let repository_installer = AlwaysNotFoundRepositoryInstaller;

        let mut stderr_output = Vec::new();
        let options = dependency_install_options(&test_base_dirs, &repository_installer, false)
            .expect("dependency install options should build");
        let error = ensure_dylint_tools_with_executor(
            &executor,
            options.policy,
            &mut stderr_output,
            |status, stderr| install_dylint_tools_with_options(&executor, status, stderr, options),
        )
        .expect_err("expected install failure");

        assert_cargo_dylint_install_failed(&error);
        executor.assert_finished();
    })?;
    Ok(())
}
