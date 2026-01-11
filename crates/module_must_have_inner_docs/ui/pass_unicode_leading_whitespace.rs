//! UI test: inner docs preceded by Unicode whitespace should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    // The next line begins with U+2028 (line separator) before the inner doc.
     //! Unicode whitespace should be skipped before docs.

    pub fn value() -> usize {
        7
    }
}

fn main() {
    let _ = documented::value();
}
