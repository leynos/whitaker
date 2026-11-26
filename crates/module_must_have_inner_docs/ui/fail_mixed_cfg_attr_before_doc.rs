//! UI test: one `cfg_attr` yielding a non-doc attribute before a doc is reported.
#![warn(module_must_have_inner_docs)]

mod mixed {
    #![cfg_attr(all(), warn(dead_code))]
    #![cfg_attr(all(), doc = "Documentation follows an expanded non-doc attribute.")]

    pub fn example() {}
}

fn main() {
    let _ = mixed::example();
}