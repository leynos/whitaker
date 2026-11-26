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
//!
//! The `expect_*`, `cfg_attr_expect_*` and `control_*` items measure the
//! `#[expect]` interaction recorded in the users' guide. The crate-level
//! `unknown_lints` allow exists because a Dylint lint is unknown to the plain
//! `cargo` build this example is also compiled by; the two Clippy allows exist
//! because those attributes are the measurement rather than lint hygiene.

#![allow(
    dead_code,
    reason = "the fixture exists to be compiled and inspected, not run"
)]
#![allow(
    unknown_lints,
    reason = "a plain cargo build does not know the Dylint lints this fixture names"
)]
#![allow(
    clippy::allow_attributes,
    reason = "the fixture must use allow, not expect, for the lint it is measuring"
)]
#![allow(
    clippy::allow_attributes_without_reason,
    reason = "the item-level allows are the measurement, not lint hygiene"
)]
// `rustc_private` is needed only when this example is built as a test harness:
// under `make coverage` the workspace `RUSTFLAGS` mark the dev-dependencies
// unstable, and a plain test crate may not use them without it. The other lint
// crates' example fixtures carry the same conditional.
#![cfg_attr(test, feature(rustc_private))]

// Every measurement item below exists only in the test configuration.
// `#[expect]` attributes here are *intentionally* unfulfilled in at least one
// build — the control documents itself so the lint cannot fire — so a non-test
// compilation under `-D warnings` would reject the fixture. The Dylint gate
// checks this example twice: once as a plain example and once as a test
// harness. Gating the items keeps the plain build empty and still leaves the
// harness build, which is what the UI test inspects, unchanged.
#[cfg(test)]
use rstest::rstest;
#[cfg(test)]
use rstest_bdd_macros::given;

#[cfg(test)]
#[given("a runtime alias counter initialized to 0")]
fn runtime_alias_counter_init() {}

#[cfg(test)]
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
#[cfg(test)]
#[expect(missing_docs_in_test_functions)]
const fn control_expectation_is_unfulfilled() -> u8 { 0 }

// Measurement: `#[expect]` on a procedural-attribute function. The allow is
// only there so a plain `cargo` build, which does not know the lint, accepts
// the name; the expectation itself is what is being observed.
#[cfg(test)]
#[expect(missing_docs_in_test_functions)]
#[given("an expectation on a step function")]
fn expect_on_given_step() {}

// Measurement: a conditional expectation on a procedural-attribute function.
#[cfg(test)]
#[cfg_attr(test, expect(missing_docs_in_test_functions))]
#[given("a conditional expectation on a step function")]
fn cfg_attr_expect_on_given_step() {}

// Measurement: the same form on an `#[rstest]` function, which rewrites the
// item differently from `#[given]`. Recording both keeps the guidance from
// generalizing across macros it has not observed.
#[cfg(test)]
#[expect(missing_docs_in_test_functions)]
#[rstest]
#[case(2)]
fn expect_on_rstest_case(#[case] value: i32) {
    assert_eq!(value, 2, "the case value must be carried through");
}

#[cfg(test)]
macro_rules! define_tests {
    () => {
        const fn generated_by_macro_rules() {}
    };
}

#[cfg(test)]
define_tests!();

/// Entry point, documented so it is not reported.
fn main() {}
