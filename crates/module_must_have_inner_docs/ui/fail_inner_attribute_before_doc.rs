#![warn(module_must_have_inner_docs)]

mod reordering {
    #![allow(dead_code)]
    //! Documentation arrives too late.

    pub fn example() {}
}

fn main() {
    reordering::example();
}
