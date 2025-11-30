//! UI test: panic_any inside `unwrap_or_else` should be linted.

#![deny(no_unwrap_or_else_panic)]

fn main() {
    let value: Option<i32> = None;
    let _ = value.unwrap_or_else(|| std::panic::panic_any("boom"));
}
