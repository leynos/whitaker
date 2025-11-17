#![warn(module_must_have_inner_docs)]

mod documented {
    //! Describe the module purpose before any other statements.

    pub fn value() -> usize {
        42
    }
}

fn main() {
    let _ = documented::value();
}
