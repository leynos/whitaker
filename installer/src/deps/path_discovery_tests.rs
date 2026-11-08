//! Tests for PATH-based binary discovery.

#[cfg(unix)]
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::{ambient_authority, fs_utf8::Dir};
use temp_env::with_vars_unset;

use super::{
    PathScanFailure,
    PathScanFailureCategory,
    PathScanResult,
    is_binary_on_path,
    is_executable_file,
    scan_path_directories,
};
use crate::{
    test_support::env_test_guard,
    test_utils::dependency_binary_helpers::{with_fake_path, write_fake_binary},
};

// Assertions live in these helpers because the tests below return `Result` to
// propagate fixture errors, and `panic_in_result_fn` denies asserting inside
// such a function.

fn assert_binary_present(binary_name: &str) {
    assert!(
        is_binary_on_path(binary_name),
        "{binary_name} should resolve from PATH"
    );
}

#[cfg(windows)]
fn assert_binary_state(binary_name: &str, expected: bool) {
    assert_eq!(
        is_binary_on_path(binary_name),
        expected,
        "unexpected PATH resolution for {binary_name}"
    );
}

fn assert_binary_absent(binary_name: &str) {
    assert!(
        !is_binary_on_path(binary_name),
        "{binary_name} should not resolve from PATH"
    );
}

fn assert_scan_found(scan: &PathScanResult, expected: &std::path::Path) {
    assert_eq!(
        scan.binary.as_deref(),
        Some(expected),
        "the scan should find the staged executable"
    );
}

fn assert_scan_has_no_failures(scan: &PathScanResult) {
    assert!(
        scan.failures.is_empty(),
        "no PATH entry should be reported: {:?}",
        scan.failures
    );
}

#[cfg(unix)]
fn assert_executable_matches(
    directory: &cap_std::fs_utf8::Dir,
    name: &camino::Utf8Path,
    expected: bool,
) {
    assert_eq!(
        is_executable_file(directory, name, &mut Vec::new()),
        expected,
        "the executable permission bit should decide classification"
    );
}

#[cfg(unix)]
fn assert_scan_classified(scan: &PathScanResult, expected: &std::path::Path) {
    assert_scan_found(scan, expected);
    assert_eq!(
        scan.failures,
        vec![
            PathScanFailure {
                binary_name: "dylint-link".to_owned(),
                category: PathScanFailureCategory::NonUtf8Path,
            },
            PathScanFailure {
                binary_name: "dylint-link".to_owned(),
                category: PathScanFailureCategory::DirectoryOpen,
            },
        ],
        "invalid PATH entries should be classified in scan order"
    );
}

#[test]
fn is_binary_on_path_returns_false_when_path_is_unset() {
    let _guard = env_test_guard();
    with_vars_unset(["PATH"], || {
        assert_binary_absent("dylint-link");
    });
}

#[test]
fn is_binary_on_path_returns_false_when_path_is_empty() {
    let _guard = env_test_guard();
    temp_env::with_var("PATH", Some(""), || {
        assert_binary_absent("dylint-link");
    });
}

#[test]
fn is_binary_on_path_returns_false_when_binary_is_missing_from_all_directories()
-> std::io::Result<()> {
    with_fake_path(
        |_| Ok(()),
        || {
            assert_binary_absent("dylint-link");
        },
    )?;
    Ok(())
}

#[test]
fn is_binary_on_path_checks_multiple_directories() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let second_dir = directories.get(1).expect("fake PATH has two directories");
            #[cfg(windows)]
            let binary_path = second_dir.join("dylint-link.exe");
            #[cfg(not(windows))]
            let binary_path = second_dir.join("dylint-link");

            write_fake_binary(&binary_path, true)
        },
        || {
            assert_binary_present("dylint-link");
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

    assert_executable_matches(
        &directory,
        Utf8Path::new("dylint-link"),
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

    assert_scan_classified(&scan, &expected_binary);
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

    assert_scan_found(&scan, &expected_binary);
    assert_scan_has_no_failures(&scan);
    Ok(())
}

/// Skip a PATH directory that disappeared and continue to a later executable.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixtures cannot be created.
#[test]
fn path_scan_skips_missing_directories_and_returns_a_later_binary() -> std::io::Result<()> {
    let fixture_root = tempfile::tempdir()?;
    let missing_directory = fixture_root.path().join("removed-from-path");
    let valid_directory = tempfile::tempdir()?;
    let binary_name = "dylint-link.exe";
    let expected_binary = valid_directory.path().join(binary_name);
    write_fake_binary(&expected_binary, true)?;

    let scan = scan_path_directories(
        vec![missing_directory, valid_directory.path().to_path_buf()],
        binary_name,
    );

    assert_scan_found(&scan, &expected_binary);
    assert_scan_has_no_failures(&scan);
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
                    binary_name: "probe.exe".to_owned(),
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
        |directories| {
            let directory = directories
                .get(dir_index)
                .expect("fake PATH has the requested directory");
            write_fake_binary(&directory.join(binary_name), true)
        },
        || {
            assert_binary_state(binary_name, expected);
        },
    )?;
    Ok(())
}

/// Detect `dylint-link` through a suffix supplied by the Windows `PATHEXT`.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[cfg(windows)]
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let directory = directories.first().expect("fake PATH has one directory");
            write_fake_binary_with_status(&directory.join("dylint-link.cmd"), true, 0)
        },
        || {
            temp_env::with_var("PATHEXT", Some(".CMD;.BAT"), || {
                let executor = StubExecutor::new(vec![
                    cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
                    dylint_link_install_list_check()
                        .expect("dylint-link dependency version should resolve"),
                ]);

                let status = check_dylint_tools(&executor)
                    .expect("available dependency checks should complete");

                assert_status(status, both_tools_available());
                executor.assert_finished();
            });
        },
    )?;
    Ok(())
}
