//! UI test: an outer attribute on the first child item is not inner documentation.
#![warn(module_must_have_inner_docs)]

mod outer_attr {
    #[doc = "This is an outer doc attribute, not an inner one."]
    pub fn example() {}
}

fn main() {
    outer_attr::example();
}
