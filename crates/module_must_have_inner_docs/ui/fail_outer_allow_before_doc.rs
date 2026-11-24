//! UI test: an outer attribute on the module declaration is not inner documentation.
#![warn(module_must_have_inner_docs)]

#[allow(dead_code)]
mod declared_with_outer_attribute {
    pub fn example() {}
}

fn main() {
    declared_with_outer_attribute::example();
}
