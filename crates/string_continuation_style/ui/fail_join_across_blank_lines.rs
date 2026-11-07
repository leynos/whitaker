// run-rustfix
#![warn(string_continuation_style)]

//! A join that skips blank source lines is still reported.
//!
//! The escaped newline consumes the blank line and the indentation after it,
//! so the evaluated value keeps one space. The pass still reports the join,
//! and the fixed file proves the rewrite preserves the value.

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "alpha \
    
        beta";
const _: () = assert!(support::same_bytes(VALUE.as_bytes(), b"alpha beta"));

fn main() {}
