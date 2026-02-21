//! UI fixture: non-test functions may include examples without warnings.
#![warn(test_must_not_have_example)]

/// Helper used to prove example docs are accepted for non-test functions.
///
/// # Examples
/// ```
/// helper();
/// // Outcome: this fixture compiles cleanly because `helper` is not test-like.
/// ```
fn helper() {}

fn main() {
    helper();
}
