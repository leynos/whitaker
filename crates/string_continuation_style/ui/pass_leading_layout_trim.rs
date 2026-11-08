#![deny(string_continuation_style)]

//! A leading layout trim is layout, not a fragment boundary.
//!
//! The continuation opens the literal, so it trims the indentation before the
//! first line rather than joining two fragments. Rewriting it as `concat!()`
//! would misrepresent the source shape, so the literal keeps its continuation.
//! Control: fail_plain_join.rs

const VALUE: &str = "\
    first line\n    second line";

fn main() {
    assert_eq!(VALUE, "first line\n    second line");
}
