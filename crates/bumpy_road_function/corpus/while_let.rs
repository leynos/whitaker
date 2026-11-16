//! Syntax matrix: an `if` nested in a `while let`.
//!
//! `nested_while_let` holds two separate `while let` regions each containing an
//! `if`. `flat_while_let` holds two sequential `while let` loops, no nesting.

/// Hold two separate `while let` regions with a nested `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_while_let(mut left: Vec<i32>, mut right: Vec<i32>) -> i32 {
    let mut total = 0;
    while let Some(item) = left.pop() {
        if item > 0 {
            total += item;
        }
    }
    while let Some(item) = right.pop() {
        if item > 0 {
            total -= item;
        }
    }
    total
}

/// Hold two sequential `while let` loops and no nested conditional.
fn flat_while_let(mut left: Vec<i32>, mut right: Vec<i32>) -> i32 {
    let mut total = 0;
    while let Some(item) = left.pop() {
        total += item;
    }
    while let Some(item) = right.pop() {
        total -= item;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (
        nested_while_let(vec![1, 2], vec![3]),
        flat_while_let(vec![1, 2], vec![3]),
    );
}
