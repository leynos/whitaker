// Plain comment, as the Welsh fixture `include!`s this file: the lint rejects doc comments after attributes.
#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

#[inline]
/// Function doc comment appears after `#[inline]`.
fn function_doc_after_attribute() {}

struct Demo;

impl Demo {
    #[allow(dead_code)]
    /// Method doc comment appears after `#[allow]`.
    fn method_doc_after_attribute(&self) {}
}

trait TraitDemo {
    #[allow(dead_code)]
    /// Trait method doc comment appears after `#[allow]`.
    fn trait_doc_after_attribute(&self);
}

fn main() {}
