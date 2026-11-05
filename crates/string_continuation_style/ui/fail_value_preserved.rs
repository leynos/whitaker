// run-rustfix
#![warn(string_continuation_style)]

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "tab\tvalue \
    joined";
const _: () = assert!(support::same_bytes(VALUE.as_bytes(), b"tab\tvalue joined"));

fn main() {}
