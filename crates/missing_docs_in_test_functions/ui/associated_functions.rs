//! Associated functions are in scope only at module level in an inherent or
//! trait declaration.
//!
//! The inherent method and the trait's default method are reported. The
//! trait-implementation method is not: its documentation conventionally lives
//! on the trait, and `missing_docs` makes the same exception. The bodiless
//! required trait method is not reported either, because it has no body to
//! document.

#![allow(dead_code)]

struct Widget;

impl Widget {
    fn inherent_method(&self) {}
}

trait Behaviour {
    fn required_method(&self);

    fn default_method(&self) {}
}

impl Behaviour for Widget {
    fn required_method(&self) {}

    fn default_method(&self) {}
}

/// Entry point, documented so it is not reported.
fn main() {}