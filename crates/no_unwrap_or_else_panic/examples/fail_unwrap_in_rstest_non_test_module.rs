//! Negative regression example for rstest harness detection.
//!
//! The real rstest expansion in this crate is covered by
//! `pass_unwrap_in_rstest_harness`. This example keeps an unrelated same-named
//! companion module next to ordinary code to ensure arbitrary const-only
//! modules are not mistaken for rstest harness descriptors during `--test`
//! builds.

#![cfg_attr(
    test,
    feature(rustc_private),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]

use rstest::rstest;

#[expect(dead_code, reason = "example fixture not used at runtime")]
fn parse() {
    let parsed = std::iter::once("value").next();
    let _ = parsed.unwrap_or_else(|| {
        let ordinary: Option<&str> = None;
        #[expect(
            clippy::unnecessary_literal_unwrap,
            reason = "fixture uses literal Option unwrap in test scenario"
        )]
        #[expect(
            clippy::unwrap_used,
            reason = "intentional panic to test unwrap detection in fixture"
        )]
        ordinary.unwrap()
    });
}

/// Const-only sibling module used as a fixture.
///
/// Verifies that `collect_rstest_companion_test_functions` does not mistake an
/// arbitrary `const`-only module for a synthesised rstest harness descriptor.
/// A genuine rstest companion module contains `#[test]` functions generated
/// by the proc-macro; a module containing only `pub const` items must never
/// be treated as one.
#[expect(dead_code, reason = "example fixture not used at runtime")]
mod parse {
    pub const VERSION: &str = "1";
}

#[rstest]
#[case("value")]
fn unrelated_rstest_harness(#[case] value: &str) {
    assert_eq!(value, "value");
}

fn main() {}
