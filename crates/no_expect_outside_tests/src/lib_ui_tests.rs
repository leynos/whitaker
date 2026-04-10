//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use dylint_testing::ui::Test;
use rstest::rstest;
use temp_env::with_vars_unset;
use whitaker_common::test_support::{env_test_guard, run_test_runner};

fn run_example_under_test_harness(example_name: &str, label: &str) {
    run_example_under_test_harness_with_flags(example_name, label, &["--test"]);
}

fn run_example_under_test_harness_with_flags(
    example_name: &str,
    label: &str,
    rustc_flags: &[&str],
) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(example_name, || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test = Test::example(crate_name, example_name);
                    test.rustc_flags(rustc_flags);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "{label} example regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

#[rstest]
#[case("pass_expect_in_tokio_test_harness", "Tokio")]
#[case("pass_expect_in_rstest_harness", "rstest")]
fn example_compiles_under_test_harness(#[case] example_name: &str, #[case] label: &str) {
    run_example_under_test_harness(example_name, label);
}

#[test]
fn rstest_expect_outside_tests_still_fails_in_non_harness_code() {
    run_example_under_test_harness_with_flags(
        "fail_expect_in_rstest_non_test_module",
        "rstest non-harness",
        &["--test", "-D", "no_expect_outside_tests"],
    );
}
