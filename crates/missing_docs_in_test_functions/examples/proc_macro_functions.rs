//! Procedural attribute macros are inspected before they rewrite the function.
//!
//! `#[rstest]` and `rstest-bdd`'s `#[given]` both consume the function they
//! decorate during expansion and emit wrappers and scenario machinery in its
//! place. A post-expansion lint cannot tell the authored function from the
//! generated ones; this pass runs before any of that, so each function is
//! reported exactly once at its own identifier.
//!
//! The `macro_rules!` invocation at the bottom is the control for the opposite
//! case: functions written *inside* a macro invocation are token trees, never
//! items, so they are never reported.

#![allow(
    dead_code,
    reason = "the fixture exists to be compiled and inspected, not run"
)]
// `rustc_private` is needed only when this example is built as a test harness:
// under `make coverage` the workspace `RUSTFLAGS` mark the dev-dependencies
// unstable, and a plain test crate may not use them without it. The other lint
// crates' example fixtures carry the same conditional.
#![cfg_attr(test, feature(rustc_private))]

use rstest::rstest;
use rstest_bdd_macros::given;

#[given("a runtime alias counter initialized to 0")]
fn runtime_alias_counter_init() {}

#[rstest]
#[case(1)]
fn case_driven_test(#[case] value: i32) {
    assert_eq!(value, 1, "the case value must be carried through");
}

/// Positive control, documented so the lint genuinely does not fire.
///
/// Its expectation therefore can never be fulfilled, so
/// `unfulfilled_lint_expectations` must be reported. This proves the control
/// mechanism works: without it, silence on the `#[given]` cases below could be
/// read as "the expectation was fulfilled" rather than "nothing was observed".
#[allow(unknown_lints)]
#[expect(missing_docs_in_test_functions)]
fn control_expectation_is_unfulfilled() -> u8 { 0 }

// Measurement: `#[expect]` on a procedural-attribute function. The allow is
// only there so a plain `cargo` build, which does not know the lint, accepts
// the name; the expectation itself is what is being observed.
#[allow(unknown_lints)]
#[expect(missing_docs_in_test_functions)]
#[given("an expectation on a step function")]
fn expect_on_given_step() {}

// Measurement: a conditional expectation on a procedural-attribute function.
#[allow(unknown_lints)]
#[cfg_attr(test, expect(missing_docs_in_test_functions))]
#[given("a conditional expectation on a step function")]
fn cfg_attr_expect_on_given_step() {}

// Measurement: the same form on an `#[rstest]` function, which rewrites the
// item differently from `#[given]`. Recording both keeps the guidance from
// generalizing across macros it has not observed.
#[allow(unknown_lints)]
#[expect(missing_docs_in_test_functions)]
#[rstest]
#[case(2)]
fn expect_on_rstest_case(#[case] value: i32) {
    assert_eq!(value, 2, "the case value must be carried through");
}

macro_rules! define_tests {
    () => {
        const fn generated_by_macro_rules() {}
    };
}

define_tests!();

/// Entry point, documented so it is not reported.
fn main() {}
