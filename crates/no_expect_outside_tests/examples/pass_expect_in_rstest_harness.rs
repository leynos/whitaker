//! Regression example covering real `#[rstest]` test harness lowering for
//! `no_expect_outside_tests`.
//!
//! The case-driven test exercises the proc-macro expansion shape that keeps the
//! original function body separate from the generated harness descriptors.

#![cfg_attr(test, deny(no_expect_outside_tests))]

use rstest::rstest;

#[rstest]
#[case(1)]
fn pass_expect_in_rstest_harness(#[case] value: i32) {
    let parsed = Some(value).expect("case-driven rstest functions should count as tests");
    assert_eq!(parsed, 1);
}

// `rstest` 0.26.1 does not export the legacy `#[rstest_parametrize]` macro, so
// this regression covers the real `#[rstest]` + `#[case]` lowering shape
// exercised by current projects while the shared attribute registry continues to
// recognize the legacy name.

fn main() {}
