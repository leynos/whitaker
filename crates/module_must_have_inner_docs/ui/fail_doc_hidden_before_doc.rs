//! UI test: `#![doc(hidden)]` before real documentation is the first inner attribute.
#![warn(module_must_have_inner_docs)]

mod misordered {
    #![doc(hidden)]
    //! Documentation arrives after a non-doc inner attribute.

    pub fn example() {}
}

fn main() {
    let _ = misordered::example();
}
