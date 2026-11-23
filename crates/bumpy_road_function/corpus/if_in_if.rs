//! Syntax matrix: an `if` nested in an `if`.
//!
//! `nested_if` holds two separate `if`-in-`if` regions, so a structural
//! detector reports two. `flat_if` holds four sequential, unnested `if`
//! statements and reports none.

/// Hold two separate `if`-in-`if` regions.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_if(a: i32, b: i32) -> i32 {
    let mut total = 0;
    if a > 0 {
        if b > 0 {
            total += 1;
        }
    }
    total += 10;
    if a < 0 {
        if b < 0 {
            total -= 1;
        }
    }
    total
}

/// Hold four sequential `if` statements and no nesting.
fn flat_if(a: i32, b: i32) -> i32 {
    let mut total = 0;
    if a > 0 {
        total += 1;
    }
    if b > 0 {
        total += 2;
    }
    if a < 0 {
        total -= 1;
    }
    if b < 0 {
        total -= 2;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (nested_if(1, 2), flat_if(1, 2));
}
