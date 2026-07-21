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
// crate-wide suppression.
#[allow(
    unknown_lints,
    reason = "Dylint lint is unknown to plain rustc; see item comment"
)]
#[deny(no_unwrap_or_else_panic)]
fn aliased_test_crate_non_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("aliased non-companion {value}"));
}

// The UI harness compiles this fixture to assert the emitted diagnostic but
// never calls it. Reference it in an anonymous const so `dead_code` stays
// honest without an `#[expect]` suppression; this does not change what the lint
// reports on the function body.
#[cfg(test)]
const _: fn(i32) = aliased_test_crate_non_companion_subject;

/// An ordinary sibling module that happens to import the compiler test crate.
#[cfg(test)]
mod aliased_test_crate_non_companion_subject {
    extern crate test as test_harness;

    fn unrelated_item() {
        let _ = test_harness::black_box(1);
    }

    // Reference the sibling item for the same reason as the parent fixture.
    const _: fn() = unrelated_item;
}

fn main() {}
