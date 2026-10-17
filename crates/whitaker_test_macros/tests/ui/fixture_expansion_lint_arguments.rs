//! Compile-fail trybuild fixture verifying rejection of invalid
//! `allow_fixture_expansion_lints` arguments.
//!
//! The invalid invocation exercises the diagnostic contract for
//! `allow_fixture_expansion_lints` while keeping the failure isolated from the
//! passing fixture.

use whitaker_test_macros::allow_fixture_expansion_lints;

#[allow_fixture_expansion_lints(unexpected)]
fn fixture() {}

fn main() {
    fixture();
}
