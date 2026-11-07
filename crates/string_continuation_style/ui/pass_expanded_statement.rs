#![deny(string_continuation_style)]

//! A joined literal inside an expanded statement is not reported.
//!
//! The joined literal is a `$value:literal` capture replayed inside a
//! statement transcribed from the macro body, so it sits at wrapper depth
//! one. Rewriting it would also stop the matcher binding, so it stays silent.
//! Control: fail_plain_join.rs

macro_rules! expression_statement {
    ($value:literal) => {
        $value;
    };
}

fn main() {
    expression_statement!("alpha \
        beta");
}
