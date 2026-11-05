#![deny(string_continuation_style)]

macro_rules! literal_only {
    ($value:literal) => {
        let captured: &str = $value;
        assert_eq!(captured, "alpha beta");
    };
}

fn main() {
    literal_only!("alpha \
        beta");
}
