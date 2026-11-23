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

#![allow(dead_code)]
#![allow(unexpected_cfgs)]

use rstest::rstest;
use rstest_bdd_macros::given;

#[given("a runtime alias counter initialized to 0")]
fn runtime_alias_counter_init() {}

#[rstest]
#[case(1)]
fn case_driven_test(#[case] value: i32) {
    assert_eq!(value, 1, "the case value must be carried through");
}

macro_rules! define_tests {
    () => {
        fn generated_by_macro_rules() {}
    };
}

define_tests!();

/// Entry point, documented so it is not reported.
fn main() {}
