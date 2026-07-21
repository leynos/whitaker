//! Trybuild bootstrap input for the `rstest_helper_should_be_fixture` UI suite.
//!
//! The integration harness compiles this standalone source to preserve
//! zero-diagnostic coverage independently of the Cargo-backed example path.

use rstest::rstest;

fn main() {}

#[rstest]
#[case::single_use("helper input")]
fn rstest_macro_fixture_compiles_without_diagnostics(#[case] value: &str) {
    assert_eq!(value, "helper input");
}
