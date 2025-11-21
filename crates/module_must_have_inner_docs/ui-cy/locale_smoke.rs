//! Welsh locale smoke test for `module_must_have_inner_docs` diagnostics.
#![warn(module_must_have_inner_docs)]

mod missing {
    pub fn demo() {}
}

fn main() {
    missing::demo();
}
