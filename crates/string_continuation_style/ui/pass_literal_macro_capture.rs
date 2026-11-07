#![deny(string_continuation_style)]

//! A `$literal` capture keeps its continuation.
//!
//! `literal_only!` matches `$value:literal` and replays the token in a
//! transcribed `let`, so the literal sits at wrapper depth one. Replacing it
//! with `concat!()` would stop the matcher binding, so the pass stays silent.
//! Control: fail_plain_join.rs

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
