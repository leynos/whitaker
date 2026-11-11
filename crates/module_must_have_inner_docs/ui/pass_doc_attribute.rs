//! UI test: `#![doc = "…"]` should pass.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![doc = "Attribute documentation describes the module purpose."]

    pub fn value() -> usize {
        5
    }
}

fn main() {
    let _ = documented::value();
}