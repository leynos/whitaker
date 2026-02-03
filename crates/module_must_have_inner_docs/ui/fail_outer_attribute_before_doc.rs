//! UI test: an outer attribute appearing before module docs must trigger the lint.
#![warn(module_must_have_inner_docs)]

mod outer_attr {
    #[doc = "This is an outer doc attribute, not an inner one."]
    pub fn example() {}
}

fn main() {
    outer_attr::example();
}
