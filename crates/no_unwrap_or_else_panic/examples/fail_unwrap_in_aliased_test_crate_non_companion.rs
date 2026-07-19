//! Negative regression for an aliased `test` crate without rstest descriptors.
//!
//! An aliased `test` import alone is not structural evidence of an rstest
//! companion module. The lint must still report the parent function.

#![cfg_attr(
    test,
    feature(rustc_private, test),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]

#[cfg(test)]
#[expect(
    dead_code,
    reason = "fixture body is exercised by the Dylint UI compile only"
)]
fn aliased_test_crate_non_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("aliased non-companion {value}"));
}

/// An ordinary sibling module that happens to import the compiler test crate.
#[cfg(test)]
mod aliased_test_crate_non_companion_subject {
    extern crate test as test_harness;

    #[expect(dead_code, reason = "fixture establishes an unrelated module item")]
    fn unrelated_item() {
        let _ = test_harness::black_box(1);
    }
}

fn main() {}
