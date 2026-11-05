// run-rustfix
#![warn(string_continuation_style)]

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "left \
    middle \
    right";
const _: () = assert!(support::same_bytes(VALUE.as_bytes(), b"left middle right"));

fn main() {}
