//! Negative regression example for rstest harness detection.
//!
//! The real rstest expansion in this crate is covered by
//! `pass_expect_in_rstest_harness`. This example keeps an unrelated same-named
//! companion module next to ordinary code to ensure arbitrary const-only
//! modules are not mistaken for rstest harness descriptors during `--test`
//! builds.

use rstest::rstest;

#[expect(dead_code, reason = "example fixture not used at runtime")]
#[expect(
    clippy::expect_used,
    reason = "the fixture deliberately calls expect outside test-only code"
)]
fn parse() {
    let parsed = std::iter::once("value").next();
    let _ = parsed.expect("ordinary code must not inherit rstest harness status");
}

#[expect(dead_code, reason = "example fixture not used at runtime")]
mod parse {
    pub const VERSION: &str = "1";
}

#[rstest]
#[case("value")]
fn unrelated_rstest_harness(#[case] value: &str) {
    assert_eq!(value, "value");
}

fn main() {}
