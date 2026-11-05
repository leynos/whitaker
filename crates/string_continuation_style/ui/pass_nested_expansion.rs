#![deny(string_continuation_style)]

macro_rules! inner_value {
    ($value:literal) => {{
        let inner: &str = $value;
        inner
    }};
}

macro_rules! outer_value {
    ($value:literal) => {{
        let outer: &str = inner_value!($value);
        outer
    }};
}

const VALUE: &str = outer_value!("alpha \
    beta");

fn main() {
    assert_eq!(VALUE, "alpha beta");
}
