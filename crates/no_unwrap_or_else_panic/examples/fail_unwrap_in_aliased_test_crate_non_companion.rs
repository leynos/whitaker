//! Negative regression for an aliased `test` crate without rstest descriptors.
//!
//! An aliased `test` import alone is not structural evidence of an rstest
//! companion module. The lint must still report the parent function.

#![cfg_attr(test, feature(rustc_private, test))]

// The Dylint UI harness compiles this fixture with `-D no_unwrap_or_else_panic`
// on the command line, so the lint is registered and denied there without an
// in-source lint attribute that plain rustc would reject as an unknown lint.
#[cfg(test)]
#[expect(
    dead_code,
    reason = "compiled by the Dylint UI harness solely to assert the emitted lint; never invoked. Migration to #[whitaker_support::dylint_expect] is tracked by roadmap item 2.2.9."
)]
fn aliased_test_crate_non_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("aliased non-companion {value}"));
}

/// An ordinary sibling module that happens to import the compiler test crate.
#[cfg(test)]
mod aliased_test_crate_non_companion_subject {
    extern crate test as test_harness;

    #[expect(
        dead_code,
        reason = "proves an aliased `test` import alone is not an rstest companion; never invoked. Migration to #[whitaker_support::dylint_expect] is tracked by roadmap item 2.2.9."
    )]
    fn unrelated_item() {
        let _ = test_harness::black_box(1);
    }
}

fn main() {}
