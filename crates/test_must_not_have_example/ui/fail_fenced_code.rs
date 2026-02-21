//! UI fixture: emits a warning for fenced code in test-like docs.
#![warn(test_must_not_have_example)]

/// Use this fixture to verify fenced-block detection for test-like docs.
/// Outcome: the lint reports a fenced-code violation.
/// ```rust
/// assert!(true);
/// ```
#[expect(
    dead_code,
    reason = "Fixture helper exists solely to exercise lint diagnostics"
)]
fn fail_fenced_code() {
    assert!(true);
}

fn main() {}
