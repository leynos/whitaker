//! Negative regression for an aliased `test` crate without rstest descriptors.
//!
//! An aliased `test` import alone is not structural evidence of an rstest
//! companion module. The lint must still report the parent function.

#![cfg_attr(test, feature(rustc_private, test))]

#[cfg(test)]
// `no_unwrap_or_else_panic` is a Dylint lint that plain rustc does not register
// when it compiles this example as a `--test` target during `cargo test`.
// Scoping the `allow(unknown_lints)` to this one item keeps the `deny` active
// under the Dylint driver (and the harness's `-D` flag) while avoiding a
// crate-wide suppression. Tracked by roadmap item 2.2.9.
#[allow(
    unknown_lints,
    reason = "Dylint lint is unknown to plain rustc; see item comment"
)]
#[deny(no_unwrap_or_else_panic)]
#[expect(
    dead_code,
    reason = "the Dylint UI harness compiles this fixture only to assert the emitted diagnostics; nothing calls it, so the compiler cannot see a use. Migration to `#[whitaker_support::dylint_expect(...)]` is tracked by roadmap item 2.2.9."
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
        reason = "fixture proves an aliased `test` import alone is not an rstest companion; the item is never invoked. Migration to `#[whitaker_support::dylint_expect(...)]` is tracked by roadmap item 2.2.9."
    )]
    fn unrelated_item() {
        let _ = test_harness::black_box(1);
    }
}

fn main() {}
