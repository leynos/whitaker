//! UI test: safe `unwrap_or_else` should be allowed inside #[test].
#![deny(no_unwrap_or_else_panic)]

#[test]
fn allows_safe_fallbacks_in_tests() {
    let value: Result<i32, &str> = Err("boom");
    let _ = value.unwrap_or_else(|_| 42);
}

fn main() {}
