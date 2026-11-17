//! Syntax matrix: an `if` nested in an `if let`.
//!
//! `nested_if_let` holds two separate `if let` regions each containing an `if`.
//! `flat_if_let` holds four sequential `if let` statements with no nesting.

/// Hold two separate `if let` regions with a nested `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_if_let(a: Option<i32>, b: Option<i32>) -> i32 {
    let mut total = 0;
    if let Some(left) = a {
        if left > 0 {
            total += left;
        }
    }
    total += 10;
    if let Some(right) = b {
        if right > 0 {
            total += right;
        }
    }
    total
}

/// Hold four sequential `if let` statements and no nesting.
fn flat_if_let(a: Option<i32>, b: Option<i32>) -> i32 {
    let mut total = 0;
    if let Some(left) = a {
        total += left;
    }
    if let Some(right) = b {
        total += right;
    }
    if let Some(left) = b {
        total -= left;
    }
    if let Some(right) = a {
        total -= right;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (nested_if_let(Some(1), Some(2)), flat_if_let(Some(1), None));
}
