//! Cargo-backed bootstrap input for the Dylint UI harness.
//!
//! `tests/ui.rs`'s `ExampleHarness` compiles this example through
//! `dylint_testing::ui::Test::example` to retain zero-diagnostic coverage for
//! basic `#[rstest]` macro expansion.
#![feature(rustc_private)]

use rstest::rstest;

fn main() {}

#[rstest]
#[case::single_use("helper input")]
fn rstest_macro_fixture_compiles_without_diagnostics(#[case] value: &str) {
    assert_eq!(value, "helper input");
}
