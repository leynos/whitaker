#![deny(string_continuation_style)]

//! Attribute meta-item literals are not replaceable expressions.
//!
//! The wrapper-depth walk must not record a span for the literal inside
//! `#[doc = "..."]`, because the lint hook cannot rewrite an attribute value.
//! A source-line join inside an attribute therefore passes silently. The
//! paired control lives in `fail_join_beside_attribute.rs`, which carries an
//! attribute and an ordinary expression in the same item.

#[doc = "alpha \
    beta"]
pub struct Documented;

fn main() { let _ = Documented; }
