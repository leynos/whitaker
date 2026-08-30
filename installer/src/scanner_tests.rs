//! Tests for scanning installed lint libraries.

use rstest::rstest;
use tempfile::TempDir;

use super::*;

/// Skip test execution on non-Linux platforms where library extensions differ.
macro_rules! skip_unless_linux {
    () => {
        if !cfg!(target_os = "linux") {
            return;
        }
    };
}

#[rstest]
#[case::standard_linux(
    "libmodule_max_lines@nightly-2026-05-28.so",
    "module_max_lines",
    "nightly-2026-05-28"
)]
#[case::suite(
    "libwhitaker_suite@nightly-2026-05-28.so",
    "whitaker_suite",
    "nightly-2026-05-28"
)]
#[case::stable_toolchain(
    "libno_expect_outside_tests@stable-1.80.0.so",
    "no_expect_outside_tests",
    "stable-1.80.0"
)]
fn parse_library_filename_valid(
    #[case] filename: &str,
    #[case] expected_crate: &str,
    #[case] expected_toolchain: &str,
) {
    skip_unless_linux!();

    let result = parse_library_filename(filename);
    assert!(result.is_some(), "expected Some for {filename}");

    let (crate_name, toolchain) = result.expect("already checked");
    assert_eq!(crate_name.as_str(), expected_crate);
    assert_eq!(toolchain, expected_toolchain);
}

#[rstest]
#[case::no_at_sign("libmodule_max_lines.so")]
#[case::empty_crate("lib@nightly-2026-05-28.so")]
#[case::empty_toolchain("libmodule_max_lines@.so")]
#[case::wrong_prefix("module_max_lines@nightly-2026-05-28.so")]
#[case::wrong_extension("libmodule_max_lines@nightly-2026-05-28.dll")]
#[case::random_file("readme.txt")]
fn parse_library_filename_invalid(#[case] filename: &str) {
    skip_unless_linux!();

    let result = parse_library_filename(filename);
    assert!(result.is_none(), "expected None for {filename}");
}

#[test]
fn lints_for_suite_returns_standard_lints_only() {
    let lints = lints_for_library(&CrateName::from("whitaker_suite"));
    // Suite reports only standard lints; experimental lints depend on build flags
    assert_eq!(lints.len(), LINT_CRATES.len());

    for lint in LINT_CRATES {
        assert!(lints.contains(lint), "missing standard lint: {lint}");
    }
    for lint in EXPERIMENTAL_LINT_CRATES {
        assert!(
            !lints.contains(lint),
            "suite should not report experimental lint: {lint}"
        );
    }
}

#[test]
fn lints_for_suite_includes_experimental_when_requested() {
    let lints = lints_for_library_with_experimental(&CrateName::from("whitaker_suite"), true);

    assert_eq!(
        lints.len(),
        LINT_CRATES.len() + EXPERIMENTAL_LINT_CRATES.len(),
        "suite should report standard and experimental lints"
    );
    for lint in LINT_CRATES {
        assert!(lints.contains(lint), "missing standard lint: {lint}");
    }
    for lint in EXPERIMENTAL_LINT_CRATES {
        assert!(
            lints.contains(lint),
            "suite should include experimental lint when requested: {lint}"
        );
    }
}

#[test]
fn lints_for_individual_crate_returns_single_lint() {
    let lints = lints_for_library(&CrateName::from("module_max_lines"));
    assert_eq!(lints, vec!["module_max_lines"]);
}

#[test]
fn lints_for_bumpy_road_crate_returns_single_lint() {
    let lints = lints_for_library(&CrateName::from("bumpy_road_function"));
    assert_eq!(lints, vec!["bumpy_road_function"]);
}

#[test]
fn lints_for_unknown_crate_returns_empty() {
    let lints = lints_for_library(&CrateName::from("unknown_crate"));
    assert!(lints.is_empty());
}

#[test]
fn scan_empty_directory_returns_empty() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let target_dir = Utf8Path::from_path(temp.path()).expect("non-UTF8 path");

    let result = scan_installed(target_dir).expect("scan should succeed");
    assert!(result.is_empty());
}

#[test]
fn scan_nonexistent_directory_returns_empty() {
    let result = scan_installed(Utf8Path::new("/nonexistent/path")).expect("scan should succeed");
    assert!(result.is_empty());
}

#[test]
fn scan_finds_installed_libraries() {
    skip_unless_linux!();

    let temp = TempDir::new().expect("failed to create temp dir");
    let target_dir = Utf8Path::from_path(temp.path()).expect("non-UTF8 path");

    // Create toolchain directory structure
    let toolchain = "nightly-2026-05-28";
    let release_dir = target_dir.join(toolchain).join("release");
    std::fs::create_dir_all(&release_dir).expect("failed to create dirs");

    // Create fake library files
    let lib_name = format!("libwhitaker_suite@{toolchain}.so");
    std::fs::write(release_dir.join(&lib_name), b"fake").expect("failed to write file");

    let result = scan_installed(target_dir).expect("scan should succeed");
    assert!(!result.is_empty());
    assert!(result.by_toolchain.contains_key(toolchain));

    let libs = result
        .by_toolchain
        .get(toolchain)
        .expect("toolchain should exist");
    assert_eq!(libs.len(), 1);
    let library = libs.first().expect("library should be recorded");
    assert_eq!(library.crate_name.as_str(), "whitaker_suite");
    assert_eq!(library.toolchain, toolchain);
}
