// run-rustfix
#![warn(string_continuation_style)]

fn main() {
    let value = 7;
    assert!(
        value > 0,
        "alpha \
         beta{}",
        value
    );
}
