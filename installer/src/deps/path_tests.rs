//! Tests for the dependency-availability query against a fake PATH.

use super::*;
use crate::test_utils::{
    ExpectedCall,
    StubExecutor,
    dependency_binary_helpers::{
        cargo_dylint_check,
        cargo_dylint_check_with_result,
        dylint_link_install_list_check,
        dylint_link_install_list_check_with_version,
        with_fake_binary_on_path,
        with_fake_path,
        write_fake_binary_with_status,
    },
    stdout_output,
};

// Assertions live in these helpers because the tests below return `Result` to
// propagate fixture errors, and `panic_in_result_fn` denies asserting inside
// such a function.

/// The status reported when both Dylint tools resolve.
///
/// Shared with the `path_scan::path_discovery_tests` module, whose Windows
/// `PATHEXT` regression asserts the same status shape. That module is a child
/// of `path_scan` rather than of `deps`, so `pub(super)` here would not reach
/// it.
pub(crate) fn both_tools_available() -> DylintToolStatus {
    DylintToolStatus {
        cargo_dylint: true,
        dylint_link: true,
    }
}

/// The status reported when only `cargo-dylint` resolves.
fn cargo_dylint_only() -> DylintToolStatus {
    DylintToolStatus {
        cargo_dylint: true,
        dylint_link: false,
    }
}

/// The status reported when neither tool resolves.
fn no_tools_available() -> DylintToolStatus {
    DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    }
}

/// Assert the observed status matches the expected one.
///
/// Shared with the `path_scan::path_discovery_tests` module for the Windows
/// `PATHEXT` regression, which reports the same comparison. `pub(crate)` rather
/// than `pub(super)` because that module is a descendant of `path_scan`, not a
/// sibling of this one.
pub(crate) fn assert_status(status: DylintToolStatus, expected: DylintToolStatus) {
    assert_eq!(status, expected, "unexpected Dylint tool status");
}

fn assert_version_parse_error(error: &impl std::fmt::Display) {
    assert!(
        error
            .to_string()
            .contains("without reporting a semantic version"),
        "unparsable version output should be reported, got: {error}"
    );
}
/// Verify both installed tools are reported as available.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[test]
fn check_dylint_tools_reports_installed_tools() -> std::io::Result<()> {
    with_fake_binary_on_path("dylint-link", || {
        let executor = StubExecutor::new(vec![
            cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
            dylint_link_install_list_check()
                .expect("dylint-link dependency version should resolve"),
        ]);

        let status =
            check_dylint_tools(&executor).expect("available dependency checks should complete");

        assert_status(status, both_tools_available());
        executor.assert_finished();
    })?;
    Ok(())
}

/// Reject Cargo output that does not report the pinned `cargo-dylint` version.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[rstest::rstest]
#[case::stale_version("cargo-dylint 5.0.0\n")]
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

            let status =
                check_dylint_tools(&executor).expect("available dependency checks should complete");

            assert_status(status, no_tools_available());
            executor.assert_finished();
        },
    )?;
    Ok(())
}

/// Reject successful Cargo output that contains no parseable version.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[test]
fn check_dylint_tools_rejects_unparsable_cargo_dylint_output() -> std::io::Result<()> {
    with_fake_path(
        |_| Ok(()),
        || {
            let executor = StubExecutor::new(vec![cargo_dylint_check_with_result(Ok(
                stdout_output("not a version\n"),
            ))]);

            let error = check_dylint_tools(&executor)
                .expect_err("unparsable version output must not authorize installation");

            assert_version_parse_error(&error);
            executor.assert_finished();
        },
    )?;
    Ok(())
}

/// Reject `dylint-link` when Cargo does not record the pinned installed version.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
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

        let status =
            check_dylint_tools(&executor).expect("available dependency checks should complete");

        assert_status(status, cargo_dylint_only());
        executor.assert_finished();
    })?;
    Ok(())
}

/// Verify PATH detection never executes the discovered dylint-link wrapper.
#[test]
fn check_dylint_tools_does_not_invoke_dylint_link_on_path() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let first_dir = directories.first().expect("fake PATH has one directory");
            #[cfg(windows)]
            let binary_path = first_dir.join("dylint-link.cmd");
            #[cfg(not(windows))]
            let binary_path = first_dir.join("dylint-link");

            write_fake_binary_with_status(&binary_path, true, 1)
        },
        || {
            let executor = StubExecutor::new(vec![
                cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
                dylint_link_install_list_check()
                    .expect("dylint-link dependency version should resolve"),
            ]);

            let status =
                check_dylint_tools(&executor).expect("available dependency checks should complete");

            assert_status(
                status,
                DylintToolStatus {
                    cargo_dylint: true,
                    dylint_link: true,
                },
            );
            executor.assert_finished();
        },
    )?;
    Ok(())
}
