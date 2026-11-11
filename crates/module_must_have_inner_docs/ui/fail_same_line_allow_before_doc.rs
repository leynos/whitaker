//! UI test: an inner lint attribute before same-line docs is reported.
#![warn(module_must_have_inner_docs)]

mod reordering {
    #![allow(dead_code)] #![doc = "Documentation follows the lint attribute."]

    pub fn example() {}
}

fn main() {
    let _ = reordering::example();
}