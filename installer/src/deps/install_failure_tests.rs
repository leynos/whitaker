//! Verify installer command-query errors are propagated through fallback paths.

use super::*;
use rstest::rstest;

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

/// Identify the installer query whose error must be propagated.
#[derive(Clone, Copy, Debug)]
enum QueryFailure {
    BinstallAvailability,
    PostBinstallVerification,
    PostCargoInstallVerification,
    RepositoryVerification,
    CompanionRefresh,
}

impl QueryFailure {
    /// Return the error text associated with this query failure.
    fn message(self) -> &'static str {
        match self {
            Self::BinstallAvailability => "binstall lookup failed",
            Self::PostBinstallVerification => "post-binstall lookup failed",
            Self::PostCargoInstallVerification => "post-cargo-install lookup failed",
            Self::RepositoryVerification => "repository verification lookup failed",
            Self::CompanionRefresh => "companion refresh lookup failed",
        }
    }

    /// Prepare the repository installer result that reaches this query.
    fn configure_repository_installer(self, installer: &mut MockDependencyBinaryInstaller) {
        match self {
            Self::BinstallAvailability => {
                installer.expect_install().never();
            }
            Self::RepositoryVerification => {
                installer
                    .expect_install()
                    .returning(|_, _, _| Ok(PathBuf::from("/tmp/bin/cargo-dylint")));
            }
            _ => {
                installer
                    .expect_install()
                    .returning(|_, _, _| Err(DependencyBinaryInstallError::MissingBinDir));
            }
        }
    }

    /// Build the command sequence that reaches this failing query.
    fn expected_calls(self) -> Vec<ExpectedCall> {
        if let Self::BinstallAvailability = self {
            return vec![binstall_version_check_with_result(Err(InstallerError::Io(
                std::io::Error::other(self.message()),
            )))];
        }

        let binstall_available = matches!(self, Self::PostBinstallVerification);
        let mut calls = vec![binstall_version_check_with_result(if binstall_available {
            Ok(success_output())
        } else {
            Ok(failure_output("missing binstall"))
        })];

        if binstall_available {
            calls.push(
                binstall_install("cargo-dylint", Ok(success_output()))
                    .expect("binstall install call should be configured"),
            );
        }

        if matches!(
            self,
            Self::PostCargoInstallVerification | Self::CompanionRefresh
        ) {
            calls.push(ExpectedCall {
                cmd: "cargo",
                args: vec!["install", "--locked", "--version", "6.3.0", "cargo-dylint"],
                result: Ok(success_output()),
            });
        }

        match self {
            Self::PostBinstallVerification
            | Self::PostCargoInstallVerification
            | Self::RepositoryVerification => calls.push(cargo_dylint_check_with_result(Err(
                InstallerError::Io(std::io::Error::other(self.message())),
            ))),
            Self::CompanionRefresh => {
                calls.push(cargo_dylint_check().expect("cargo-dylint verification is configured"));
                calls.push(ExpectedCall {
                    cmd: "cargo",
                    args: vec!["install", "--list"],
                    result: Err(InstallerError::Io(std::io::Error::other(self.message()))),
                });
            }
            Self::BinstallAvailability => unreachable!("handled before building calls"),
        }

        calls
    }

    /// Indicate whether this failure occurs while refreshing the companion.
    fn refreshes_companion(self) -> bool {
        matches!(self, Self::CompanionRefresh)
    }
}

/// Preserve errors from dependency queries across installation paths.
#[rstest]
#[case::binstall_availability(QueryFailure::BinstallAvailability)]
#[case::post_binstall_verification(QueryFailure::PostBinstallVerification)]
#[case::post_cargo_install_verification(QueryFailure::PostCargoInstallVerification)]
#[case::repository_verification(QueryFailure::RepositoryVerification)]
#[case::companion_refresh(QueryFailure::CompanionRefresh)]
fn dependency_query_errors_are_propagated(#[case] failure: QueryFailure) {
    let mut repository_installer = MockDependencyBinaryInstaller::new();
    failure.configure_repository_installer(&mut repository_installer);
    let executor = StubExecutor::new(failure.expected_calls());
    let mut stderr = Vec::new();

    let error = with_fake_binary_on_path("dylint-link", || {
        install_dylint_tools_with_options(
            &executor,
            &DylintToolStatus {
                cargo_dylint: false,
                dylint_link: !failure.refreshes_companion(),
            },
            &mut stderr,
            install_options(&repository_installer, false),
        )
    })
    .expect("temporary companion binary should be staged")
    .expect_err("query errors must stop installation");

    assert_spawn_error(error, failure.message());
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
            args: vec!["install", "--locked", "--version", "6.3.0", "cargo-dylint"],
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
