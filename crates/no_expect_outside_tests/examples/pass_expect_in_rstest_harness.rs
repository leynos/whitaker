//! Regression example covering real `#[rstest]` test harness lowering for
//! `no_expect_outside_tests`.
//!
//! The case-driven test exercises the proc-macro expansion shape that keeps the
//! original function body separate from the generated harness descriptors.

use rstest::rstest;

#[rstest]
fn pass_expect_in_plain_rstest_harness() {
    let parsed = Some(1).expect("plain rstest functions should count as tests");
    assert_eq!(parsed, 1);
}

#[rstest]
#[case(1)]
fn pass_expect_in_parametrized_rstest_harness(#[case] value: i32) {
    let parsed = Some(value).expect("case-driven rstest functions should count as tests");
    assert_eq!(parsed, 1);
}

fn main() {}
