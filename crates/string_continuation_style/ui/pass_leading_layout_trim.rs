#![deny(string_continuation_style)]

const VALUE: &str = "\
    first line\n    second line";

fn main() {
    assert_eq!(VALUE, "first line\n    second line");
}
