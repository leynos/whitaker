#![deny(string_continuation_style)]

//! A format string produced by `concat!` is not a source literal.
//!
//! The builtin macro sets `FormatArgs::is_source_literal` by re-lexing the
//! snippet the format-literal expression occupies and comparing it with the
//! parsed input. `concat!("alpha ", "beta")` recovers as that macro call, which
//! does not lex as one cooked string token, so the flag is false and there is
//! no direct source literal to rewrite.
//!
//! The joined literal inside `concat!` is invisible to this lint for the same
//! reason: it is matched by `concat!`'s own `$($e:expr),*` contract, so it is
//! not a format string at all.

fn main() {
    let rendered = format!(concat!("alpha ", "beta"));
    assert_eq!(rendered, "alpha beta");
}
