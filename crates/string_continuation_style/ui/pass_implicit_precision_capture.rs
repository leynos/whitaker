#![deny(string_continuation_style)]

//! A captured precision argument is an implicit capture.
//!
//! `{value:.precision$}` names `precision` without listing it as a format
//! argument. It exercises the same `Captured` variant as width through a
//! different placeholder position, so a predicate that only inspected the
//! placeholder kind rather than every argument would be caught here.

fn main() {
    let value = "truncate me";
    let precision = 4;
    let rendered = format!("alpha \
        beta {value:.precision$}");
    assert_eq!(rendered, "alpha beta trun");
}
