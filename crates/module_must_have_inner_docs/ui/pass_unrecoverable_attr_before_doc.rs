//! UI test: an attribute with no recoverable span before the docs is not reported.
//!
//! The pinned compiler keeps no user-written span for `RecursionLimit`, so the
//! adapter cannot place a diagnostic on it. This fixture pins the accepted
//! contract: the module is still treated as documented by the later `//!`.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![recursion_limit = "32"]
    //! Documentation follows an attribute whose span is unrecoverable.

    pub fn example() {}
}

fn main() {
    let _ = documented::example();
}
