//! UI test: a lone inner lint attribute is not documentation.
#![warn(module_must_have_inner_docs)]

mod undocumented {
    #![warn(dead_code)]

    pub fn example() {}
}

fn main() {
    let _ = undocumented::example();
}