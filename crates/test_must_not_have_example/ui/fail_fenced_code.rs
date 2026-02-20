//! UI fixture: emits a warning for fenced code in test-like docs.
#![warn(test_must_not_have_example)]

#[expect(
    dead_code,
    reason = "Fixture helper exists solely to exercise lint diagnostics"
)]
/// ```rust
/// assert!(true);
/// ```
fn fail_fenced_code() {
    assert!(true);
}

fn main() {}
