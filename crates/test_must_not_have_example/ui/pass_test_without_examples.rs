#![warn(test_must_not_have_example)]

#[allow(dead_code)]
/// Validates behaviour without embedding examples.
fn pass_test_without_examples() {
    assert!(true);
}

fn main() {}
