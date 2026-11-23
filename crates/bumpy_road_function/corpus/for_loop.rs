//! Syntax matrix: an `if` nested in a `for` loop.
//!
//! `nested_for` holds two separate `for` regions each containing an `if`.
//! `flat_for` holds two sequential `for` loops with no nested conditional.

/// Hold two separate `for` regions with a nested `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_for(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        if *item > 0 {
            total += item;
        }
    }
    for item in items {
        if *item < 0 {
            total -= item;
        }
    }
    total
}

/// Hold two sequential `for` loops and no nested conditional.
fn flat_for(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        total += item;
    }
    for item in items {
        total -= item * 2;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (nested_for(&[1, -2]), flat_for(&[1, -2]));
}
