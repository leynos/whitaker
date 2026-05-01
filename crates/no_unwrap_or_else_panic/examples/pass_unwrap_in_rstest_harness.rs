//! Regression example covering real `#[rstest]` test harness lowering for
//! `no_unwrap_or_else_panic`.
//!
//! The case-driven test exercises the proc-macro expansion shape that keeps the
//! original function body separate from the generated harness descriptors.

#![cfg_attr(
    test,
    feature(rustc_private),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]

use rstest::rstest;

#[rstest]
#[case(1)]
#[expect(
    clippy::unwrap_used,
    reason = "fixture must exercise the lint's unwrap allowance"
)]
fn pass_unwrap_in_rstest_harness(#[case] value: i32) {
    let parsed = Some(value).unwrap();
    assert_eq!(parsed, 1);
}

// This fixture covers the current `#[rstest]` + `#[case]` lowering shape.

fn main() {}
