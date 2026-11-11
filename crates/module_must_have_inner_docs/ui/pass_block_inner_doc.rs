//! UI test: block inner docs should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    /*! Block documentation describes the module purpose. */

    pub fn value() -> usize {
        4
    }
}

fn main() {
    let _ = documented::value();
}