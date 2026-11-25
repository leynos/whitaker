//! UI test: an outer attribute on the declaration does not disturb inner docs.
//!
//! The module is documented, so this passes only because the outer `#[allow]`
//! is excluded from the module's inner records. If the adapter retained it,
//! its span precedes the documentation and the lint would report
//! `FirstInnerIsNotDoc` on the attribute.
#![warn(module_must_have_inner_docs)]

#[allow(dead_code)]
mod documented {
    //! The module is documented by this inner doc comment.

    pub fn example() {}
}

fn main() {
    documented::example();
}