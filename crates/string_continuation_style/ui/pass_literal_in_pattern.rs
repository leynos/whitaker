#![deny(string_continuation_style)]

//! A literal in pattern position is never visited.
//!
//! The pass dispatches on expressions, and a match arm's pattern is not one,
//! so this join is never a rewrite candidate. The fixture pins that the walk
//! does not descend into pattern syntax by accident.
//! Control: fail_plain_join.rs

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
