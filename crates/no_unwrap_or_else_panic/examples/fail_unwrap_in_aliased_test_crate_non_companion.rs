//! Negative regression for an aliased `test` crate without rstest descriptors.
//!
//! An aliased `test` import alone is not structural evidence of an rstest
//! companion module. The lint must still report the parent function.

#![cfg_attr(test, feature(rustc_private, test))]

// The Dylint UI harness compiles this fixture with `-D no_unwrap_or_else_panic`
// on the command line, so the lint is registered and denied there without an
// in-source lint attribute that plain rustc would reject as an unknown lint.
#[cfg(test)]
fn aliased_test_crate_non_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("aliased non-companion {value}"));
}

// The harness never calls this fixture; reference it in an anonymous const so
// `dead_code` stays honest. This does not change what the lint reports on the
// function body.
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
