// aux-build: derive_literal.rs
#![deny(string_continuation_style)]

//! A derive macro's output is never rewritten.
//!
//! The literal belongs to the `impl` block that `derive_literal` emits, so its
//! span comes from an expansion rather than from code the user wrote. The
//! pass declines such spans and stays silent.
//! Control: fail_plain_join.rs

extern crate derive_literal;

use derive_literal::GenerateLiteral;

#[derive(GenerateLiteral)]
struct Generated;

fn main() {
    assert_eq!(Generated::VALUE, "alpha beta");
}
