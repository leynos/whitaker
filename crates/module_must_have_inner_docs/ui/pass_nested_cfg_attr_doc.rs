//! UI test: a nested active `cfg_attr` producing documentation should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![cfg_attr(all(), cfg_attr(all(), doc = "Nested active documentation."))]

    pub fn value() -> usize {
        7
    }
}

fn main() {
    let _ = documented::value();
}