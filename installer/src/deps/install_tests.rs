//! Tests for dependency-install status refresh behaviour.

use super::*;
use rstest::rstest;

fn dylint_link_probe_executor() -> crate::test_utils::StubExecutor {
    crate::test_utils::StubExecutor::new(vec![crate::test_utils::ExpectedCall {
        cmd: "dylint-link",
        args: vec!["--version"],
        result: Ok(crate::test_utils::success_output()),
    }])
}

#[rstest]
#[case(InstallOutcome::CargoBinstall)]
#[case(InstallOutcome::CargoInstall)]
fn update_status_after_install_refreshes_link_for_local_cargo_dylint_installs(
    #[case] outcome: InstallOutcome,
) {
    let executor = dylint_link_probe_executor();
    let mut status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    update_status_after_install(&mut status, &executor, &CARGO_DYLINT_TOOL, outcome);

    assert!(status.cargo_dylint);
    assert!(status.dylint_link);
    executor.assert_finished();
}

#[test]
fn update_status_after_install_skips_link_probe_for_repository_release() {
    let executor = crate::test_utils::StubExecutor::new(vec![]);
    let mut status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    update_status_after_install(
        &mut status,
        &executor,
        &CARGO_DYLINT_TOOL,
        InstallOutcome::RepositoryRelease,
    );

    assert!(status.cargo_dylint);
    assert!(!status.dylint_link);
    executor.assert_finished();
}

#[test]
fn should_install_tool_returns_true_for_cargo_dylint_when_not_installed() {
    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    assert!(should_install_tool(&status, &CARGO_DYLINT_TOOL));
}

#[test]
fn should_install_tool_returns_false_for_cargo_dylint_when_installed() {
    let status = DylintToolStatus {
        cargo_dylint: true,
        dylint_link: false,
    };

    assert!(!should_install_tool(&status, &CARGO_DYLINT_TOOL));
}

#[test]
fn should_install_tool_returns_true_for_dylint_link_when_not_installed() {
    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    assert!(should_install_tool(&status, &DYLINT_LINK_TOOL));
}

#[test]
fn should_install_tool_returns_false_for_dylint_link_when_installed() {
    let status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: true,
    };

    assert!(!should_install_tool(&status, &DYLINT_LINK_TOOL));
}

#[rstest]
#[case::non_repo_release_and_link_missing(
    InstallOutcome::CargoInstall,
    DylintToolStatus { cargo_dylint: false, dylint_link: false },
    true,
)]
#[case::repository_release(
    InstallOutcome::RepositoryRelease,
    DylintToolStatus { cargo_dylint: false, dylint_link: false },
    false,
)]
#[case::link_already_present(
    InstallOutcome::CargoBinstall,
    DylintToolStatus { cargo_dylint: true, dylint_link: true },
    false,
)]
fn should_refresh_companions_returns_expected(
    #[case] outcome: InstallOutcome,
    #[case] status: DylintToolStatus,
    #[case] expected: bool,
) {
    assert_eq!(should_refresh_companions(outcome, &status), expected);
}
