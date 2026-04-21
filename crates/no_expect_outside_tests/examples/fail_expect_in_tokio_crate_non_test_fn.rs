//! Negative regression ensuring Tokio-specific test configuration does not
//! leak into ordinary code.
//!
//! The fixture config recognises `#[tokio::test]`, but `parse_config` remains
//! production code and must still trigger the lint even in the same crate.

#[allow(dead_code)]
fn parse_config() {
    let parsed = std::iter::once("value").next();
    let _ = parsed.expect("ordinary code in a Tokio crate must still lint");
}

#[tokio::test]
async fn unrelated_tokio_test() {
    let value = Ok::<_, ()>("ok");
    value.expect("actual Tokio tests should still be allowed");
}

fn main() {}
