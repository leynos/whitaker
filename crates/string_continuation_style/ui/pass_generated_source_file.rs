#![deny(string_continuation_style)]

//! A literal in generated source is not reported.
//!
//! The `include!()` below pulls in a file written beneath the target
//! directory, which no user maintains and no suggestion can edit. The lint
//! requires such literals to keep their continuations and stays silent.
//! Control: fail_plain_join.rs

include!("../../../target/string_continuation_style-generated-ui.rs");

fn main() {
    assert_eq!(GENERATED, "alpha beta");
}
