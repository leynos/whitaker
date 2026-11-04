#![warn(string_continuation_style)]

const VALUE: &str = "alpha \
    beta";

fn main() {
    assert_eq!(VALUE, "alpha beta");
}
