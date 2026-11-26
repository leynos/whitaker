//! UI test: an ordinary comment before the inner docs should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    // The ordinary comment explains why the module docs follow it.
    //! Module docs may follow an ordinary comment.

    /// Return a fixed value so the fixture has module content.
    pub fn value() -> usize {
        3
    }
}

/// Run the fixture so every item is used.
fn main() {
    let _ = documented::value();
}