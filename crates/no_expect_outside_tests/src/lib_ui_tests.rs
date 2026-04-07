//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use dylint_testing::ui::Test;
use temp_env::with_vars_unset;
use whitaker_common::test_support::{env_test_guard, run_test_runner};

#[test]
fn tokio_example_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner("pass_expect_in_tokio_test_harness", || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test = Test::example(crate_name, "pass_expect_in_tokio_test_harness");
                    test.rustc_flags(["--test"]);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "Tokio example regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

/// Regression for #189: real `#[rstest]` with `#[case]` must be recognised
/// as test-only code under `--test`, validating end-to-end detection via the
/// actual rstest crate rather than the auxiliary proc-macro stub.
#[test]
fn rstest_example_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner("pass_expect_in_rstest_harness", || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test = Test::example(crate_name, "pass_expect_in_rstest_harness");
                    test.rustc_flags(["--test"]);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "rstest example regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

/// Regression for #132: `#[tokio::test]` in `#[path]`-loaded modules with
/// non-standard names must be recognised as test contexts under `--test`.
#[test]
fn path_module_tokio_test_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner("pass_expect_in_path_module_harness", || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test =
                        Test::example(crate_name, "pass_expect_in_path_module_harness");
                    test.rustc_flags(["--test"]);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "Path module harness regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}
