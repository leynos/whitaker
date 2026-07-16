//! Tests for PATH-based dependency-binary discovery helpers.

use super::*;
use crate::test_support::env_test_guard;
use crate::test_utils::dependency_binary_helpers::{
    cargo_dylint_check, cargo_dylint_check_with_result, dylint_link_install_list_check,
    dylint_link_install_list_check_with_version, with_fake_binary_on_path, with_fake_path,
    write_fake_binary, write_fake_binary_with_status,
};
use crate::test_utils::{ExpectedCall, StubExecutor, stdout_output};
use temp_env::with_vars_unset;

#[test]
fn check_dylint_tools_reports_installed_tools() {
    with_fake_binary_on_path("dylint-link", || {
        let executor =
            StubExecutor::new(vec![cargo_dylint_check(), dylint_link_install_list_check()]);

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

#[rstest::rstest]
#[case::stale_version("cargo-dylint 5.0.0\n")]
#[case::unparsable_output("not a version\n")]
fn check_dylint_tools_rejects_unusable_cargo_dylint_output(#[case] version_stdout: &str) {
    // The fake PATH keeps dylint-link absent so only cargo-dylint is probed.
    with_fake_path(
        |_| {},
        || {
            let executor = StubExecutor::new(vec![cargo_dylint_check_with_result(Ok(
                stdout_output(version_stdout),
            ))]);

            let status = check_dylint_tools(&executor);

            assert_eq!(
                status,
                DylintToolStatus {
                    cargo_dylint: false,
                    dylint_link: false,
                }
            );
            executor.assert_finished();
        },
    );
}

#[rstest::rstest]
#[case::stale_version(dylint_link_install_list_check_with_version("5.0.0"))]
#[case::missing_from_list(ExpectedCall {
    cmd: "cargo",
    args: vec!["install", "--list"],
    result: Ok(stdout_output("ripgrep v14.1.0:\n    rg\n")),
})]
fn check_dylint_tools_rejects_unpinned_dylint_link(#[case] install_list_check: ExpectedCall) {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![cargo_dylint_check(), install_list_check]);

        let status = check_dylint_tools(&executor);

        assert_eq!(
            status,
            DylintToolStatus {
                cargo_dylint: true,
                dylint_link: false,
            }
        );
        executor.assert_finished();
    });
}

#[test]
fn check_dylint_tools_rejects_non_invocable_dylint_link_on_path() {
    with_fake_path(
        |directories| {
            #[cfg(windows)]
            let binary_path = directories[0].join("dylint-link.cmd");
            #[cfg(not(windows))]
            let binary_path = directories[0].join("dylint-link");

            write_fake_binary_with_status(&binary_path, true, 1);
        },
        || {
            let executor = StubExecutor::new(vec![cargo_dylint_check()]);

            let status = check_dylint_tools(&executor);

            assert_eq!(
                status,
                DylintToolStatus {
                    cargo_dylint: true,
                    dylint_link: false,
                }
            );
            executor.assert_finished();
        },
    );
}

#[test]
fn is_binary_on_path_returns_false_when_path_is_unset() {
    let _guard = env_test_guard();
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
        |directories| {
            #[cfg(windows)]
            let binary_path = directories[1].join("dylint-link.exe");
            #[cfg(not(windows))]
            let binary_path = directories[1].join("dylint-link");

            write_fake_binary(&binary_path, true);
        },
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
#[rstest::rstest]
#[case("dylint-link.exe", 0, true)]
#[case("dylint-link", 1, false)]
fn is_binary_on_path_handles_windows_executable_suffixes(
    #[case] binary_name: &str,
    #[case] dir_index: usize,
    #[case] expected: bool,
) {
    with_fake_path(
        |directories| write_fake_binary(&directories[dir_index].join(binary_name), true),
        || {
            assert_eq!(is_binary_on_path(binary_name), expected);
        },
    );
}

#[cfg(windows)]
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() {
    with_fake_path(
        |directories| {
            write_fake_binary_with_status(&directories[0].join("dylint-link.cmd"), true, 0)
        },
        || {
            temp_env::with_var("PATHEXT", Some(".CMD;.BAT"), || {
                let executor =
                    StubExecutor::new(vec![cargo_dylint_check(), dylint_link_install_list_check()]);

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
        },
    );
}
