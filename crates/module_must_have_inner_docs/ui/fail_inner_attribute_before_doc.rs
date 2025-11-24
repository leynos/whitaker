//! UI test: an inner attribute appearing before module docs must trigger the lint.
#![warn(module_must_have_inner_docs)]

mod reordering {
    #![expect(dead_code, reason = "fixture: attribute before inner docs")]
    //! Documentation arrives too late.

    pub fn example() {}
}

fn main() {
    // Intentionally empty: `example` stays unused so the `#[expect(dead_code)]`
    // expectation is fulfilled when the dead_code lint fires.
}
