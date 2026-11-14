//! UI test: documentation behind an inactive `cfg_attr` does not count.
#![warn(module_must_have_inner_docs)]

mod undocumented {
    #![cfg_attr(any(), doc = "This documentation is inactive.")]

    pub fn example() {}
}

fn main() {
    let _ = undocumented::example();
}