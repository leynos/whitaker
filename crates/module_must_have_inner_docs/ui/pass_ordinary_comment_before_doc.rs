//! UI test: an ordinary comment before the inner docs should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    // The ordinary comment explains why the module docs follow it.
    //! Module docs may follow an ordinary comment.

    pub fn value() -> usize {
        3
    }
}

fn main() {
    let _ = documented::value();
}