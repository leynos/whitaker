// run-rustfix
#![warn(string_continuation_style)]

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "alpha \
    beta";
const _: () = assert!(support::same_bytes(VALUE.as_bytes(), b"alpha beta"));

fn main() {}
