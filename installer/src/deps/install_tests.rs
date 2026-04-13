//! Tests for dependency-install status refresh behaviour.

use super::*;
use crate::test_support::env_test_guard;
use rstest::rstest;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn with_fake_binary_on_path<T>(binary_name: &str, run: impl FnOnce() -> T) -> T {
    let _guard = env_test_guard();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let binary_path = temp_dir.path().join(binary_name);
    fs::write(&binary_path, []).expect("write fake binary");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&binary_path)
            .expect("read fake binary metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary_path, permissions).expect("mark fake binary executable");
    }

    let path = temp_dir.path().display().to_string();
    temp_env::with_var("PATH", Some(path), run)
}

#[rstest]
#[case(InstallOutcome::CargoBinstall)]
#[case(InstallOutcome::CargoInstall)]
fn update_status_after_install_refreshes_link_for_local_cargo_dylint_installs(
    #[case] outcome: InstallOutcome,
) {
    with_fake_binary_on_path("dylint-link", || {
        let executor = crate::test_utils::StubExecutor::new(vec![]);
        let mut status = DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        };

        update_status_after_install(&mut status, &executor, &CARGO_DYLINT_TOOL, outcome);

        assert!(status.cargo_dylint);
        assert!(status.dylint_link);
        executor.assert_finished();
    });
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
