// aux-build: tokio.rs
//! Positive UI fixture: allow `.expect(...)` in `#[tokio::test]` contexts.
#![deny(no_expect_outside_tests)]

#[tokio::test]
fn allows_expect_in_tokio_test() {
    let option = Some("ok");
    option.expect("tokio tests permit expect");
}

fn main() {}
