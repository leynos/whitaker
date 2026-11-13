//! UI fixture: an authored guard on a generated `match`.
//!
//! `matches!` lowers to a generated `match`, but the guard is authored, so its
//! three-conjunct condition must still be charged.
#![warn(conditional_max_n_branches)]

fn generated_match_authored_guard(value: Option<u8>) {
    let mut a = true;
    let b = true;
    let c = true;
    let _ = matches!(value, Some(_) if a && b && c);
    a = false;
    let _ = (a, b, c);
}

fn main() { generated_match_authored_guard(Some(1)); }
