//! UI test: an active non-doc `cfg_attr` before documentation is an inner attribute.
#![warn(module_must_have_inner_docs)]

fn main() {
    let _ = expanded::example();
}

mod expanded {
    #![cfg_attr(all(), warn(dead_code))]
    //! Documentation arrives after an expanded non-doc attribute.

    pub fn example() {}
}
