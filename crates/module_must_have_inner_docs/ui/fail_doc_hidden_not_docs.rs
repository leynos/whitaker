//! UI test: `#![doc(hidden)]` is not module documentation.
#![warn(module_must_have_inner_docs)]

mod undocumented {
    #![doc(hidden)]

    pub fn example() {}
}

fn main() {
    let _ = undocumented::example();
}