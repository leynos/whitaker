#![deny(string_continuation_style)]

macro_rules! define_value {
    () => {
        const GENERATED: &str = "alpha \
            beta";
    };
}

define_value!();

fn main() {
    assert_eq!(GENERATED, "alpha beta");
}
