//! UI test: safe `unwrap_or_else` fallback should not be linted.

#![deny(no_unwrap_or_else_panic)]

fn main() {
    let value: Result<i32, &str> = Err("boom");
    let _ = value.unwrap_or_else(|_| 42);
}
