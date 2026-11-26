//! UI test: a nested active `cfg_attr` producing documentation should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![cfg_attr(all(), cfg_attr(all(), doc = "Nested active documentation."))]

    /// Return a fixed value so the fixture has module content.
    pub fn value() -> usize {
        7
    }
}

/// Run the fixture so every item is used.
fn main() {
    let _ = documented::value();
}