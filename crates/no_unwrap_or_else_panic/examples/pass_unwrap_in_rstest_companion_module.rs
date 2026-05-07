//! Regression example for manual rstest companion-module lowering shape.
//!
//! Replicates the parent `fn` / sibling `mod` layout rustc emits for
//! case-driven `#[rstest]` without invoking the proc-macro. If
//! `collect_rstest_companion_test_functions` were removed, `rstest_companion_subject`
//! would no longer count as test-only and this example would trip
//! `no_unwrap_or_else_panic`.

#![cfg_attr(
    test,
    feature(rustc_private),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]

#[cfg(test)]
fn rstest_companion_subject(value: i32) {
    let parsed = Some(value);
    let _ = parsed.unwrap_or_else(|| panic!("rstest companion subject value was {value}"));
}

/// Manually-lowered rstest companion module mirroring proc-macro expansion.
///
/// Contains the `RSTEST_HARNESS_DESCRIPTOR` const and a `#[test]` function
/// that invoke the parent `rstest_companion_subject`.  Validates that
/// `collect_rstest_companion_test_functions` recognizes the sibling-module
/// layout without requiring the real `rstest` proc-macro.
#[cfg(test)]
mod rstest_companion_subject {
    pub const RSTEST_HARNESS_DESCRIPTOR: &str = "case_1";

    #[test]
    fn case_1() {
        assert_eq!(RSTEST_HARNESS_DESCRIPTOR, "case_1");
        super::rstest_companion_subject(1);
    }
}

fn main() {}
