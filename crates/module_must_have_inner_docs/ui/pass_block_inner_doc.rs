//! UI test: block inner docs should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    /*! Block documentation describes the module purpose. */

    /// Return a fixed value so the fixture has module content.
    pub fn value() -> usize {
        4
    }
}

/// Run the fixture so every item is used.
fn main() {
    let _ = documented::value();
}