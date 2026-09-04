#![deny(unused_braces)]

use whitaker_test_macros::allow_fixture_expansion_lints;

#[allow_fixture_expansion_lints]
fn fixture() -> u8 {
    { 1 }
}

fn main() {
    assert_eq!(fixture(), 1);
}
