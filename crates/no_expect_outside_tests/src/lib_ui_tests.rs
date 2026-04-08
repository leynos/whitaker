//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use dylint_testing::ui::Test;
use rstest::rstest;
use temp_env::with_vars_unset;
use whitaker_common::test_support::{env_test_guard, run_test_runner};

#[rstest]
#[case("pass_expect_in_tokio_test_harness", "Tokio example regression")]
#[case("pass_expect_in_rstest_harness", "rstest example regression")]
#[case("pass_expect_in_path_module_harness", "Path module harness regression")]
fn test_example_compiles_under_test_harness(#[case] example_name: &str, #[case] panic_msg: &str) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(example_name, || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test = Test::example(crate_name, example_name);
                    test.rustc_flags(["--test"]);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "{panic_msg} should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}
