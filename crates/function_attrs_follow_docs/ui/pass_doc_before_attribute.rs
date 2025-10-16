#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

/// Function docs appear before the inline hint.
#[inline]
fn function_doc_before_attribute() {}

struct Demo;

impl Demo {
    /// Method docs appear before `#[allow]`.
    #[allow(dead_code)]
    fn method_doc_before_attribute(&self) {}
}

trait TraitDemo {
    /// Trait docs appear before attributes.
    #[allow(dead_code)]
    fn trait_doc_before_attribute(&self);
}

fn main() {}
