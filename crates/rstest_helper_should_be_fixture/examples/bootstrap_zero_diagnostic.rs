//! Zero-diagnostic fixture for the `rstest_helper_should_be_fixture` bootstrap.
#![feature(rustc_private)]

use rstest::rstest;

fn main() {}

#[rstest]
#[case::single_use("helper input")]
fn rstest_macro_fixture_compiles_without_diagnostics(#[case] value: &str) {
    assert_eq!(value, "helper input");
}
