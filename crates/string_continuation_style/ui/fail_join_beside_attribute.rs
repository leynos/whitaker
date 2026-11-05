// run-rustfix
#![warn(string_continuation_style)]

//! Control for `pass_attribute_value.rs`.
//!
//! The item carries an attribute whose value contains a source-line join, and
//! an ordinary expression with the same shape. Skipping attribute interiors
//! must not skip the expression beside them, so exactly one warning is
//! expected here and none in `pass_attribute_value.rs`.

#[doc = "alpha \
    beta"]
pub struct Documented;

fn main() {
    let _ = Documented;
    let joined = "alpha \
        beta";
    assert_eq!(joined, "alpha beta");
}
