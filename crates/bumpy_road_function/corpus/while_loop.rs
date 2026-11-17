//! Syntax matrix: an `if` nested in a `while`.
//!
//! `nested_while` holds two separate `while` regions each containing an `if`.
//! `flat_while` holds two sequential `while` loops with no nested conditional.

/// Hold two separate `while` regions with a nested `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_while(limit: i32) -> i32 {
    let mut count = 0;
    let mut total = 0;
    while count < limit {
        if count % 2 == 0 {
            total += count;
        }
        count += 1;
    }
    count = 0;
    while count < limit {
        if count % 3 == 0 {
            total -= count;
        }
        count += 1;
    }
    total
}

/// Hold two sequential `while` loops and no nested conditional.
fn flat_while(limit: i32) -> i32 {
    let mut count = 0;
    let mut total = 0;
    while count < limit {
        total += count;
        count += 1;
    }
    count = 0;
    while count < limit {
        total -= count;
        count += 1;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (nested_while(5), flat_while(5));
}
