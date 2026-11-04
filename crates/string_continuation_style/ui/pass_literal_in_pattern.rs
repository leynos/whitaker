#![deny(string_continuation_style)]

fn classify(value: &str) -> usize {
    match value {
        "alpha \
            beta" => 1,
        _ => 0,
    }
}

fn main() {
    assert_eq!(classify("alpha beta"), 1);
}
