//! UI test: inner docs preceded by Unicode whitespace should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
     //! Module docs after Unicode whitespace (U+2028 before //!).
    // The line above starts with a literal U+2028 line separator.

    pub fn value() -> usize {
        7
    }
}

fn main() {
    let _ = documented::value();
}
