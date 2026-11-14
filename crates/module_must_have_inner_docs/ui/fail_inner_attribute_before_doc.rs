//! UI test: an inner attribute appearing before module docs must trigger the lint.
#![warn(module_must_have_inner_docs)]

mod reordering {
    #![warn(dead_code)]
    //! Documentation arrives too late.

    pub fn example() {}
}

fn main() {
    let _ = reordering::example();
}