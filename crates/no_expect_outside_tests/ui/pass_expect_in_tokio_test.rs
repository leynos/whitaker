// aux-build: tokio.rs
// edition:2021
//! Positive UI fixture: allow `.expect(...)` in `#[tokio::test]` contexts.
#![deny(no_expect_outside_tests)]

extern crate tokio;

#[tokio::test]
fn allows_expect_in_tokio_test() {
    let option = Some("ok");
    option.expect("tokio tests permit expect");
}

fn main() {}
