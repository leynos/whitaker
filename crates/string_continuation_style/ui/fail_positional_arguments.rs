// run-rustfix
#![warn(string_continuation_style)]

//! A positional format argument does not block the rewrite.
//!
//! The `{}` placeholder takes `body` positionally, so no implicit capture is
//! involved. `concat!()` leaves the argument list untouched, so the join is
//! reported and the suggestion stays valid.

#[path = "support.rs.inc"]
mod support;

fn main() {
    let body = "payload";
    let message = format!("alpha \
        beta {}", body);
    assert!(support::same_bytes(message.as_bytes(), b"alpha beta payload"));
}
