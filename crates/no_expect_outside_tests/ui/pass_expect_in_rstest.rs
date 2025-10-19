// aux-build: rstest.rs
//! Positive UI fixture: allow `.expect(...)` in `#[rstest]` contexts.
#![deny(no_expect_outside_tests)]

use rstest::rstest;

#[rstest]
fn allows_expect_in_rstest() {
    let option = Some("ok");
    option.expect("rstest contexts permit expect");
}

fn main() {}
