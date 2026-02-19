#![warn(test_must_not_have_example)]

#[allow(dead_code)]
/// # Examples
/// Avoid documenting runnable examples directly in test docs.
fn fail_examples_heading() {
    assert!(true);
}

fn main() {}
