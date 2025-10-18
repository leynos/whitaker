#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

#[doc = "Function docs declared via attribute."]
#[inline]
fn function_doc_attribute_before_attribute() {}

struct AttributeDoc;

impl AttributeDoc {
    #[doc = "Method docs declared via attribute."]
    #[allow(dead_code)]
    fn method_doc_attribute_before_attribute(&self) {}
}

trait AttributeDocTrait {
    #[doc = "Trait method docs declared via attribute."]
    #[allow(dead_code)]
    fn trait_doc_attribute_before_attribute(&self);
}

fn main() {}
