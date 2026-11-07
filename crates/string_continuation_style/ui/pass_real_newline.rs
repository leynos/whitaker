#![deny(string_continuation_style)]

//! A real newline is not a continuation.
//!
//! Only a backslash immediately before a line feed starts a continuation.
//! Here the line break is unescaped, so the value genuinely holds a newline
//! and the literal keeps its source shape.
//! Control: fail_plain_join.rs

const VALUE: &str = "alpha
beta";

fn main() {
    assert_eq!(VALUE, "alpha\nbeta");
}
