//! UI test: plain (non-interpolating) panicking `unwrap_or_else` is denied
//! inside `#[test]` because the closure does not interpolate a runtime value.
#![deny(no_unwrap_or_else_panic)]

#[test]
fn flags_plain_panicking_fallbacks_in_tests() {
    let value: Result<(), &str> = Err("boom");
    let _ = value.unwrap_or_else(|e| panic!("got: {e}"));
}

fn main() {}
