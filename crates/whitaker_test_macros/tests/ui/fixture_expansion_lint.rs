//! Passing trybuild fixture proving the fixture-expansion lint attribute works
//! with a denied `unused_braces` lint.
//!
//! The fixture applies `allow_fixture_expansion_lints` to the small function
//! whose braces would otherwise trigger the lint, covering the macro's intended
//! test-support use.

#![deny(unused_braces)]

use whitaker_test_macros::allow_fixture_expansion_lints;

#[allow_fixture_expansion_lints]
fn fixture() -> u8 {
    { 1 }
}

fn main() {
    assert_eq!(fixture(), 1);
}
