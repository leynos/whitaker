//! UI fixture: emits a warning for an examples heading in test-like docs.
#![warn(test_must_not_have_example)]

#[expect(
    dead_code,
    reason = "Fixture helper exists solely to exercise lint diagnostics"
)]
/// # Examples
/// Avoid documenting runnable examples directly in test docs.
fn fail_examples_heading() {
    assert!(true);
}

fn main() {}
