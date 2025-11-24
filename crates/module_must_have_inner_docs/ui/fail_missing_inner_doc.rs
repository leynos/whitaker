//! UI test: modules without any inner documentation should fail the lint.
#![warn(module_must_have_inner_docs)]

mod missing {
    pub fn demo() {}
}

fn main() {
    missing::demo();
}
