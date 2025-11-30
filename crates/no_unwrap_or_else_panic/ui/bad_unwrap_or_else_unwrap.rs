//! UI test: closure panics via inner `unwrap` and should be linted.

#![deny(no_unwrap_or_else_panic)]

fn main() {
    let value: Option<i32> = None;
    let _ = value.unwrap_or_else(|| {
        let nested: Option<i32> = None;
        nested.unwrap()
    });
}
