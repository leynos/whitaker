#![deny(string_continuation_style)]

//! A trailing layout trim is layout, not a fragment boundary.
//!
//! The continuation closes the literal after the last content line, so it
//! trims the indentation before the closing quote rather than joining two
//! fragments. The literal keeps it and stays silent.
//! Control: fail_plain_join.rs

const VALUE: &str = "trailing space \
    ";

fn main() {
    assert_eq!(VALUE, "trailing space ");
}
