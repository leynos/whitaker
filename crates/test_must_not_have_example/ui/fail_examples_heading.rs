//! UI fixture: emits a warning for an examples heading in test-like docs.
#![warn(test_must_not_have_example)]

/// # Examples
/// Use this fixture to verify heading detection for test-like docs.
/// Outcome: the lint reports an examples-heading violation.
#[expect(
    dead_code,
    reason = "Fixture helper exists solely to exercise lint diagnostics"
)]
fn fail_examples_heading() {
    assert!(true);
}

fn main() {}
