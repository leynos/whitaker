//! Negative regression: a hand-authored same-named `#[test]` module must not
//! exempt the parent function.
//!
//! Under `rustc --test` the `#[test]` function below gains a same-span `const`
//! harness descriptor, so the sibling module carries the exact `fn`/`const`
//! harness-descriptor pair a real rstest companion produces, together with an
//! aliased `extern crate test` that also satisfies the split-span adjacency
//! branch. The module is nonetheless hand-authored rather than emitted by the
//! `#[rstest]` proc-macro, so it carries no macro-expansion provenance;
//! `collect_rstest_companion_test_functions` must therefore reject it and the
//! lint must still fire on the parent function.

#![cfg_attr(test, feature(rustc_private, test))]

// The Dylint UI harness compiles this fixture with `-D no_unwrap_or_else_panic`
// on the command line, so the lint is registered and denied there without an
// in-source lint attribute that plain rustc would reject as an unknown lint.
#[cfg(test)]
#[expect(
    dead_code,
    reason = "compiled by the Dylint UI harness solely to assert the emitted lint; never invoked. Migration to #[whitaker_support::dylint_expect] is tracked by roadmap item 2.2.9."
)]
fn hand_written_test_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("handwritten companion {value}"));
}

/// A hand-authored same-named sibling module with the aliased harness crate and
/// a `#[test]` function. The `#[test]` gains a same-span `const` descriptor from
/// `rustc --test`, matching the harness-pair shape, but the module is not
/// macro-generated, so it must not exempt the parent.
#[cfg(test)]
mod hand_written_test_companion_subject {
    extern crate test as test_harness;

    #[test]
    fn case_1() {
        // Keeps the aliased `test` crate referenced. The parent function is left
        // uncalled so its `#[expect(dead_code)]` stays fulfilled; companion
        // detection depends on the module shape, not on the call.
        let _ = test_harness::black_box(1);
    }
}

fn main() {}
