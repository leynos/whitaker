#![warn(test_must_not_have_example)]

#[allow(dead_code)]
/// ```rust
/// assert!(true);
/// ```
fn fail_fenced_code() {
    assert!(true);
}

fn main() {}
