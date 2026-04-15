//! Tests for PATH-based dependency-binary discovery helpers.

use super::*;
use crate::test_support::env_test_guard;
use crate::test_utils::dependency_binary_helpers::{
    cargo_dylint_check_with_result, with_fake_binary_on_path, with_fake_path, write_fake_binary,
    write_fake_binary_with_status,
};
use crate::test_utils::{StubExecutor, success_output};
use temp_env::with_vars_unset;

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
            let executor =
                StubExecutor::new(vec![cargo_dylint_check_with_result(Ok(success_output()))]);

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
fn is_binary_on_path_ignores_files_without_executable_suffix() {
    with_fake_path(
        |directories| write_fake_binary(&directories[1].join("dylint-link"), true),
        || {
            assert!(!is_binary_on_path("dylint-link"));
        },
    );
}

#[cfg(windows)]
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() {
    let _guard = env_test_guard();
    temp_env::with_var("PATHEXT", Some(".CMD;.BAT"), || {
        with_fake_path(
            |directories| {
                write_fake_binary_with_status(&directories[0].join("dylint-link.cmd"), true, 0)
            },
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
