#![deny(string_continuation_style)]

//! A literal two expansions deep is not reported.
//!
//! `outer_value!` forwards its `$value` to `inner_value!`, so the literal
//! arrives through two transcribed layers. A rewrite would break both
//! `$literal` matchers, so the pass requires the continuation instead.
//! Control: fail_plain_join.rs

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
