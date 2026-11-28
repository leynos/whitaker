// run-rustfix
#![warn(string_continuation_style)]

//! Ten joins in one literal still yield one diagnostic.
//!
//! The classifier property covers any positive join count; this fixture pins
//! the emission boundary at a count well above the two-join case.

#[path = "support.rs.inc"]
mod support;

const VALUE: &str = "f0 \
    f1 \
    f2 \
    f3 \
    f4 \
    f5 \
    f6 \
    f7 \
    f8 \
    f9 \
    f10";
const _: () = assert!(support::same_bytes(
    VALUE.as_bytes(),
    b"f0 f1 f2 f3 f4 f5 f6 f7 f8 f9 f10"
));

fn main() {}