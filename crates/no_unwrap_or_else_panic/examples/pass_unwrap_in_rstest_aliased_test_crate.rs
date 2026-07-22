//! Regression fixture for an rstest companion that aliases the `test` crate.

#![cfg_attr(test, feature(rustc_private, test))]

#[cfg(test)]
fn rstest_aliased_test_crate_subject(value: i32) {
    let parsed = Some(value);
    let _ = parsed.unwrap_or_else(|| panic!("rstest companion subject value was {value}"));
}

/// Companion module with the aliased harness crate and split expansion spans.
#[cfg(test)]
mod rstest_aliased_test_crate_subject {
    extern crate test as test_harness;

    #[test]
    fn case_1() {
        let _ = test_harness::black_box(1);
        super::rstest_aliased_test_crate_subject(1);
    }
}

fn main() {}
