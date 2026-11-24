//! Windows-only PATH discovery tests: executable suffixes and `PATHEXT`.
//!
//! Split from `path_discovery_tests` to keep that module under the file-size
//! limit. The environment is injected through the stub executor, never the
//! process.

use std::ffi::OsStr;

use super::is_binary_on_path;
use crate::{
    deps::{
        check_dylint_tools,
        path_tests::{assert_status, both_tools_available},
    },
    test_utils::{
        StubExecutor,
        dependency_binary_helpers::{
            cargo_dylint_check,
            dylint_link_install_list_check,
            with_fake_path,
            write_fake_binary,
            write_fake_binary_with_status,
        },
    },
};

/// The suffix the `PATHEXT` tests inject. It is absent from the default list,
/// so finding a binary through it proves the injected value was read.
const CUSTOM_SUFFIX: &str = ".CUSTOM";

fn assert_binary_state(path: &OsStr, binary_name: &str, expected: bool) {
    let executor = StubExecutor::new(Vec::new()).with_env_var("PATH", path);
    assert_eq!(
        is_binary_on_path(&executor, binary_name),
        expected,
        "unexpected PATH resolution for {binary_name}"
    );
}

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
        |path| {
            assert_binary_state(path, binary_name, expected);
        },
    )?;
    Ok(())
}

/// An injected `PATHEXT` replaces the default list rather than extending it.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[test]
fn a_default_suffix_is_not_probed_when_pathext_is_injected() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let directory = directories.first().expect("fake PATH has one directory");
            write_fake_binary_with_status(&directory.join("dylint-link.cmd"), true, 0)
        },
        |path| {
            let executor = StubExecutor::new(Vec::new())
                .with_env_var("PATH", path)
                .with_env_var("PATHEXT", CUSTOM_SUFFIX);

            assert!(
                !is_binary_on_path(&executor, "dylint-link"),
                "a .cmd file must not match when PATHEXT names only {CUSTOM_SUFFIX}"
            );
        },
    )?;
    Ok(())
}

/// Detect `dylint-link` through a suffix supplied by the Windows `PATHEXT`.
///
/// # Errors
///
/// Returns an I/O error if the temporary PATH fixture cannot be created.
#[test]
fn check_dylint_tools_detects_dylint_link_via_pathext_suffix() -> std::io::Result<()> {
    with_fake_path(
        |directories| {
            let directory = directories.first().expect("fake PATH has one directory");
            write_fake_binary_with_status(&directory.join("dylint-link.custom"), true, 0)
        },
        |path| {
            let executor = StubExecutor::new(vec![
                cargo_dylint_check().expect("cargo-dylint dependency version should resolve"),
                dylint_link_install_list_check()
                    .expect("dylint-link dependency version should resolve"),
            ])
            .with_env_var("PATH", path)
            .with_env_var("PATHEXT", CUSTOM_SUFFIX);

            let status =
                check_dylint_tools(&executor).expect("available dependency checks should complete");

            assert_status(status, both_tools_available());
            executor.assert_finished();
        },
    )?;
    Ok(())
}
