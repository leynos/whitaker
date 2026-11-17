//! Syntax matrix: `else if` chains.
//!
//! `flat_else_if` holds two long `else if` chains with flat arms; each chain is
//! one conditional family. `else_if_with_nested_if` holds two chains where one
//! branch of each nests an `if`.

/// Hold two `else if` chains with flat branches.
fn flat_else_if(a: i32, b: i32) -> i32 {
    let first = if a > 30 {
        1
    } else if a > 20 {
        2
    } else if a > 10 {
        3
    } else {
        4
    };
    let second = if b > 30 {
        5
    } else if b > 20 {
        6
    } else if b > 10 {
        7
    } else {
        8
    };
    first + second
}

/// Hold two `else if` chains where one branch of each nests an `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn else_if_with_nested_if(a: i32, b: i32) -> i32 {
    let mut total = 0;
    if a > 30 {
        total += 1;
    } else if a > 20 {
        if b > 0 {
            total += 2;
        }
    } else {
        total += 3;
    }
    if b > 30 {
        total += 4;
    } else if b > 20 {
        if a > 0 {
            total += 5;
        }
    } else {
        total += 6;
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (flat_else_if(1, 2), else_if_with_nested_if(25, 25));
}
