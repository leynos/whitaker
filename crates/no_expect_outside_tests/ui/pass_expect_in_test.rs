//! Positive UI fixture: allow `.expect(...)` in `#[test]` contexts.
#![deny(no_expect_outside_tests)]

#[test]
fn allows_expect_in_tests() {
    let option = Some("ok");
    option.expect("test context permits expect");
}

fn main() {}
