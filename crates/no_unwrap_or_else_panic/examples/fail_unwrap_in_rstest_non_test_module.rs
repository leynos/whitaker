//! Negative regression example for rstest harness detection.
//!
//! The real rstest expansion in this crate is covered by
//! `pass_unwrap_in_rstest_harness`. This example keeps an unrelated same-named
//! companion module next to ordinary code to ensure arbitrary const-only
//! modules are not mistaken for rstest harness descriptors during `--test`
//! builds.

#![cfg_attr(
    test,
    feature(rustc_private),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]
// Negative fixture: intentional panicking `unwrap` inside the `unwrap_or_else`
// closure for UI coverage. Workspace Clippy denies would otherwise reject this
// shape.
#![allow(clippy::unnecessary_literal_unwrap, clippy::unwrap_used)]

use rstest::rstest;

#[allow(dead_code)]
fn parse() {
    let parsed = std::iter::once("value").next();
    let _ = parsed.unwrap_or_else(|| {
        let ordinary: Option<&str> = None;
        ordinary.unwrap()
    });
}

#[allow(dead_code)]
mod parse {
    pub const VERSION: &str = "1";
}

#[rstest]
#[case("value")]
fn unrelated_rstest_harness(#[case] value: &str) {
    assert_eq!(value, "value");
}

fn main() {}
