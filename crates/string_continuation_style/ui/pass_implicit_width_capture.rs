#![deny(string_continuation_style)]

//! A captured width argument is an implicit capture.
//!
//! `{value:width$}` names `width` without listing it as a format argument, so
//! the parsed placeholder carries a `FormatArgumentKind::Captured` entry.
//! Rewriting the format string to `concat!()` would disable that capture and
//! stop the program compiling.
//! Control: fail_explicit_format_arguments.rs

fn main() {
    let value = 7;
    let width = 4;
    let rendered = format!("alpha \
        beta {value:width$}");
    assert_eq!(rendered, "alpha beta    7");
}
