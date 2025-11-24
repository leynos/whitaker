//! UI test: an inner attribute appearing before module docs must trigger the lint.
#![warn(module_must_have_inner_docs)]

mod reordering {
    #![expect(
        dead_code,
        reason = "fixture intentionally places attribute before docs"
    )]
    //! Documentation arrives too late.

    pub fn example() {}
}

fn main() {
    // Intentionally empty: `example` remains unused so the `#[expect(dead_code, ..)]`
    // attribute has a matching lint.
}
