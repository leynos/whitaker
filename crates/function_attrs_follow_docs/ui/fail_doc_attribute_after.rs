#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

#[inline]
#[doc = "Function doc attribute appears after `#[inline]`."]
fn function_doc_attribute_after_attribute() {}

struct AttributeDoc;

impl AttributeDoc {
    #[allow(dead_code)]
    #[doc = "Method doc attribute appears after `#[allow]`."]
    fn method_doc_attribute_after_attribute(&self) {}
}

trait AttributeDocTrait {
    #[allow(dead_code)]
    #[doc = "Trait doc attribute appears after `#[allow]`."]
    fn trait_doc_attribute_after_attribute(&self);
}

fn main() {}
