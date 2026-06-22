//! UI harness for the `rstest_helper_should_be_fixture` lint.
//!
//! These fixtures execute the lint driver, which keeps the current
//! diagnostic-silent contract while exercising passive call-site collection.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use dylint_testing::ui::Test;
use rstest::rstest;
use whitaker_common::test_support::run_test_runner;

#[rstest]
#[case("bootstrap_zero_diagnostic")]
#[case("collection_zero_diagnostic")]
fn example_compiles_without_diagnostics(#[case] example: &str) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(example, || {
            let mut test = Test::example(crate_name, example);
            test.rustc_flags(["--test"]);
            test.run();
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
        )
    });
}
