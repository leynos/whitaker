#![deny(string_continuation_style)]

include!("../../../target/string_continuation_style-generated-ui.rs");

fn main() {
    assert_eq!(GENERATED, "alpha beta");
}
