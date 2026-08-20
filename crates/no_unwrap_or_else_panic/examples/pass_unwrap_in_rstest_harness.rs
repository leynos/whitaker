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

#[cfg(not(windows))]
use rstest::rstest;

#[cfg(not(windows))]
#[rstest]
#[case(1)]
fn pass_unwrap_in_rstest_harness(#[case] value: i32) {
    let parsed = Some(value).unwrap_or_else(|| panic!("case-driven rstest value was {value}"));
    assert_eq!(parsed, 1);
}

#[cfg(windows)]
fn pass_unwrap_in_rstest_harness(value: i32) {
    let parsed = Some(value).unwrap_or_else(|| panic!("case-driven rstest value was {value}"));
    assert_eq!(parsed, 1);
}

// Windows CI has hung while expanding `rstest` through the Dylint compiletest
// driver. Use the equivalent post-expansion companion-module shape there.
/// Manually-lowered companion module used on Windows CI.
///
/// Replicates the sibling-module shape that the `rstest` proc-macro would
/// synthesize for the `#[case(1)]` expansion.  Present only on Windows,
/// where the rstest proc-macro compilation hangs through the Dylint
/// compiletest driver.
#[cfg(all(test, windows))]
mod pass_unwrap_in_rstest_harness {
    #[test]
    fn case_1() {
        super::pass_unwrap_in_rstest_harness(1);
    }
}

fn main() {}
