// run-rustfix
#![warn(string_continuation_style)]

#[path = "support.rs.inc"]
mod support;

fn main() {
    let body = "payload";
    let message = format!("alpha \
        beta {body}", body = body);
    assert!(support::same_bytes(message.as_bytes(), b"alpha beta payload"));
}
