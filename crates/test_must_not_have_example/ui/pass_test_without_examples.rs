//! UI fixture: test-like docs without examples should not trigger warnings.
#![warn(test_must_not_have_example)]

/// Use this fixture to verify test docs without example patterns.
/// Outcome: the lint emits no diagnostic for the helper.
#[expect(
    dead_code,
    reason = "Fixture helper exists solely to validate a no-warning path"
)]
fn pass_test_without_examples() {
    assert!(true);
}

fn main() {}
