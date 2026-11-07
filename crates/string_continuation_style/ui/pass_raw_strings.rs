#![deny(string_continuation_style)]

//! Raw strings never contain a continuation.
//!
//! In `r"..."` the backslash is an ordinary character and the line break is
//! part of the value. There is no escaped newline to rewrite, so the
//! classifier ignores raw literals and the fixture asserts the literal bytes.
//! Control: fail_plain_join.rs

const RAW: &str = r"alpha \
    beta";

fn main() {
    assert!(RAW.contains('\\'));
    assert!(RAW.contains('\n'));
}
