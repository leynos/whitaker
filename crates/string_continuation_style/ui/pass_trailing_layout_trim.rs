#![deny(string_continuation_style)]

const VALUE: &str = "trailing space \
    ";

fn main() {
    assert_eq!(VALUE, "trailing space ");
}
