//! Tests for PATH-based dependency-binary discovery helpers.

// These imports are used only by the Unix executable-permission test; gating
// them keeps Windows test builds free of unused-import warnings.
#[cfg(unix)]
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::{ambient_authority, fs_utf8::Dir};
use temp_env::with_vars_unset;

use super::*;
use crate::{
    test_support::env_test_guard,
    test_utils::{
        ExpectedCall, StubExecutor,
        dependency_binary_helpers::{
            cargo_dylint_check, cargo_dylint_check_with_result, dylint_link_install_list_check,
            dylint_link_install_list_check_with_version, with_fake_binary_on_path, with_fake_path,
            write_fake_binary, write_fake_binary_with_status,
        },
        stdout_output,
    },
};

#[test]
fn check_dylint_tools_reports_installed_tools() -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
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
    })?;
    Ok(())
}

#[rstest::rstest]
#[case::stale_version("cargo-dylint 5.0.0\n")]
#[case::unparsable_output("not a version\n")]
fn check_dylint_tools_rejects_unusable_cargo_dylint_output(
    #[case] version_stdout: &str,
) -> std::io::Result<()> {
    // The fake PATH keeps dylint-link absent so only cargo-dylint is probed.
    with_fake_path(
        |_| Ok(()),
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
    )?;
    Ok(())
}

#[rstest::rstest]
#[case::stale_version(dylint_link_install_list_check_with_version("5.0.0"))]
#[case::missing_from_list(ExpectedCall {
    cmd: "cargo",
    args: vec!["install", "--list"],
    result: Ok(stdout_output("ripgrep v14.1.0:\n    rg\n")),
})]
fn check_dylint_tools_rejects_unpinned_dylint_link(
    #[case] install_list_check: ExpectedCall,
) -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
            install_list_check,
        ]);

        let status = check_dylint_tools(&executor);

        assert_eq!(
            status,
            DylintToolStatus {
                cargo_dylint: true,
                dylint_link: false,
            }
        );
        executor.assert_finished();
    })?;
    Ok(())
}

#[test]
fn check_dylint_tools_rejects_non_invocable_dylint_link_on_path() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let first_dir = &directories[0];
            #[cfg(windows)]
            let binary_path = first_dir.join("dylint-link.cmd");
            #[cfg(not(windows))]
            let binary_path = first_dir.join("dylint-link");

            write_fake_binary_with_status(&binary_path, true, 1)
        },
        || {
            let executor = StubExecutor::new(vec![
                cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
            ]);

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
    )?;
    Ok(())
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
fn is_binary_on_path_returns_false_when_binary_is_missing_from_all_directories()
-> std::io::Result<()> {
    with_fake_path(
        |_| Ok(()),
        || {
            assert!(!is_binary_on_path("dylint-link"));
        },
    )?;
    Ok(())
}

#[test]
fn is_binary_on_path_checks_multiple_directories() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let second_dir = &directories[1];
            #[cfg(windows)]
            let binary_path = second_dir.join("dylint-link.exe");
            #[cfg(not(windows))]
            let binary_path = second_dir.join("dylint-link");

            write_fake_binary(&binary_path, true)
        },
        || {
            assert!(is_binary_on_path("dylint-link"));
        },
    )?;
    Ok(())
}

/// Verify executable permission bits determine Unix candidate classification.
///
/// # Errors
///
/// Returns an I/O error if the temporary candidate cannot be created.
#[cfg(unix)]
#[rstest::rstest]
#[case::non_executable(false, false)]
#[case::executable(true, true)]
fn is_executable_file_matches_executable_permission(
    #[case] has_execute_permission: bool,
    #[case] expected_is_executable: bool,
) -> std::io::Result<()> {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let binary_path = temp_dir.path().join("dylint-link");
    write_fake_binary(&binary_path, has_execute_permission)?;

    let directory = Dir::open_ambient_dir(
        Utf8Path::from_path(temp_dir.path()).expect("temp directory path should be UTF-8"),
        ambient_authority(),
    )
    .expect("open temp-dir capability");

    assert_eq!(
        is_executable_file(&directory, Utf8Path::new("dylint-link"), &mut Vec::new(),),
        expected_is_executable,
    );
    Ok(())
}

/// Verify skipped invalid entries are classified and do not stop later searches.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixtures cannot be created.
#[cfg(unix)]
#[test]
fn path_scan_classifies_invalid_entries_and_continues_to_a_later_binary() -> std::io::Result<()> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let temp_dir = tempfile::tempdir()?;
    let mut invalid_path_bytes = temp_dir.path().as_os_str().as_bytes().to_vec();
    invalid_path_bytes.push(0xff);
    let non_utf8_entry = std::path::PathBuf::from(std::ffi::OsString::from_vec(invalid_path_bytes));

    let not_a_directory = temp_dir.path().join("not-a-directory");
    std::fs::write(&not_a_directory, b"not a directory")?;

    let valid_directory = tempfile::tempdir()?;
    let expected_binary = valid_directory.path().join("dylint-link");
    write_fake_binary(&expected_binary, true)?;

    let scan = scan_path_directories(
        vec![
            non_utf8_entry,
            not_a_directory,
            valid_directory.path().to_path_buf(),
        ],
        "dylint-link",
    );

    assert_eq!(scan.binary, Some(expected_binary));
    assert_eq!(
        scan.failures,
        vec![
            PathScanFailure {
                binary_name: "dylint-link",
                category: PathScanFailureCategory::NonUtf8Path,
            },
            PathScanFailure {
                binary_name: "dylint-link",
                category: PathScanFailureCategory::DirectoryOpen,
            },
        ]
    );
    Ok(())
}

/// Verify missing candidates are quiet and the first executable wins.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixtures cannot be created.
#[test]
fn path_scan_skips_missing_candidates_and_returns_the_first_binary() -> std::io::Result<()> {
    let empty_directory = tempfile::tempdir()?;
    let first_binary_directory = tempfile::tempdir()?;
    let later_binary_directory = tempfile::tempdir()?;
    let binary_name = "dylint-link.exe";
    let expected_binary = first_binary_directory.path().join(binary_name);
    write_fake_binary(&expected_binary, true)?;
    write_fake_binary(&later_binary_directory.path().join(binary_name), true)?;

    let scan = scan_path_directories(
        vec![
            empty_directory.path().to_path_buf(),
            first_binary_directory.path().to_path_buf(),
            later_binary_directory.path().to_path_buf(),
        ],
        binary_name,
    );

    assert_eq!(scan.binary, Some(expected_binary));
    assert!(scan.failures.is_empty());
    Ok(())
}

/// Check first-match and stop-after-match behaviour for bounded PATH patterns.
#[test]
fn path_scan_ordering_holds_for_bounded_match_patterns() {
    use proptest::prelude::*;

    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        failure_persistence: None,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x51_7a_00),
        ..proptest::test_runner::Config::default()
    });
    runner
        .run(&prop::collection::vec(0_u8..3, 1..=5), |entry_kinds| {
            let mut roots = Vec::new();
            let directories = entry_kinds
                .iter()
                .map(|entry_kind| {
                    let directory = tempfile::tempdir().expect("create PATH fixture directory");
                    let entry = if *entry_kind == 0 {
                        directory.path().to_path_buf()
                    } else if *entry_kind == 1 {
                        let file = directory.path().join("not-a-directory");
                        std::fs::write(&file, b"not a directory")
                            .expect("create non-directory PATH entry");
                        file
                    } else {
                        write_fake_binary(&directory.path().join("probe.exe"), true)
                            .expect("create executable PATH fixture");
                        directory.path().to_path_buf()
                    };
                    roots.push(directory);
                    entry
                })
                .collect::<Vec<_>>();
            let expected_binary =
                directories
                    .iter()
                    .zip(&entry_kinds)
                    .find_map(|(directory, entry_kind)| {
                        (*entry_kind == 2).then(|| directory.join("probe.exe"))
                    });
            let expected_failures = entry_kinds
                .iter()
                .take_while(|entry_kind| **entry_kind != 2)
                .filter(|entry_kind| **entry_kind == 1)
                .map(|_| PathScanFailure {
                    binary_name: "probe.exe",
                    category: PathScanFailureCategory::DirectoryOpen,
                })
                .collect::<Vec<_>>();

            let scan = scan_path_directories(directories, "probe.exe");

            prop_assert_eq!(scan.binary, expected_binary);
            prop_assert_eq!(scan.failures, expected_failures);
            drop(roots);
            Ok(())
        })
        .expect("ordered PATH scan invariant should hold for generated layouts");
}

#[cfg(windows)]
#[rstest::rstest]
#[case("dylint-link.exe", 0, true)]
#[case("dylint-link", 1, false)]
fn is_binary_on_path_handles_windows_executable_suffixes(
    #[case] binary_name: &str,
    #[case] dir_index: usize,
    #[case] expected: bool,
) -> std::io::Result<()> {
    with_fake_path(
        |directories| write_fake_binary(&directories[dir_index].join(binary_name), true),
        || {
            assert_eq!(is_binary_on_path(binary_name), expected);
        },
    )?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            write_fake_binary_with_status(&directories[0].join("dylint-link.cmd"), true, 0)
        },
        || {
            temp_env::with_var("PATHEXT", Some(".CMD;.BAT"), || {
                let executor = StubExecutor::new(vec![
                    cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
                    dylint_link_install_list_check()
                        .expect("dylint-link dependency version should resolve"),
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
            });
        },
    )?;
    Ok(())
}
