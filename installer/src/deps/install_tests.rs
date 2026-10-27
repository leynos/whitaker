//! Tests for dependency-install status refresh behaviour.

use rstest::rstest;

use super::*;
use crate::test_utils::dependency_binary_helpers::{
    dylint_link_install_list_check,
    with_fake_binary_on_path,
};

/// Verify local Cargo installs refresh companion link status.
#[rstest]
#[case(InstallOutcome::CargoBinstall)]
#[case(InstallOutcome::CargoInstall)]
fn update_status_after_install_refreshes_link_for_local_cargo_dylint_installs(
    #[case] outcome: InstallOutcome,
) -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        // The companion refresh confirms the dylint-link version against
        // Cargo's installed-binary registry after the PATH probe passes.
        let executor = crate::test_utils::StubExecutor::new(vec![
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
        ]);
        let mut status = DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        };

        update_status_after_install(&mut status, &executor, &CARGO_DYLINT_TOOL, outcome)
            .expect("local Cargo install status should refresh");

        assert!(status.cargo_dylint);
        assert!(status.dylint_link);
        executor.assert_finished();
    })?;
    Ok(())
}

/// Verify repository releases skip the companion link query.
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
    )
    .expect("repository release status should refresh");

    assert!(status.cargo_dylint);
    assert!(!status.dylint_link);
    executor.assert_finished();
}

/// Verify installation decisions for both dependency tool statuses.
#[rstest]
#[case(false, false, &CARGO_DYLINT_TOOL, true)]
#[case(true, false, &CARGO_DYLINT_TOOL, false)]
#[case(false, false, &DYLINT_LINK_TOOL, true)]
#[case(false, true, &DYLINT_LINK_TOOL, false)]
fn should_install_tool_returns_expected(
    #[case] cargo_dylint: bool,
    #[case] dylint_link: bool,
    #[case] tool: &DependencyTool,
    #[case] expected: bool,
) {
    let status = DylintToolStatus {
        cargo_dylint,
        dylint_link,
    };

    assert_eq!(should_install_tool(&status, tool), expected);
}

/// Verify companion refresh decisions for install outcomes and current status.
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
