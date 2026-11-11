//! UI test: documentation produced by an active `cfg_attr` should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![cfg_attr(all(), doc = "Active cfg_attr documentation describes the module.")]

    pub fn value() -> usize {
        6
    }
}

fn main() {
    let _ = documented::value();
}