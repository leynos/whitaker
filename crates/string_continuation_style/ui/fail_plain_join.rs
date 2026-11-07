// run-rustfix
#![warn(string_continuation_style)]

//! The baseline case: one interior join in a plain cooked string.
//!
//! The backslash-newline hides a fragment boundary inside a single literal,
//! so the pass suggests `concat!()`. A compile-time assertion in the fixed
//! file proves the rewrite preserves the evaluated bytes.

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "alpha \
    beta";
const _: () = assert!(support::same_bytes(VALUE.as_bytes(), b"alpha beta"));

fn main() {}
