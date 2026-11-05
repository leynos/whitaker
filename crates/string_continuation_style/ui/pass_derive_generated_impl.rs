// aux-build: derive_literal.rs
#![deny(string_continuation_style)]

extern crate derive_literal;

use derive_literal::GenerateLiteral;

#[derive(GenerateLiteral)]
struct Generated;

fn main() {
    assert_eq!(Generated::VALUE, "alpha beta");
}
