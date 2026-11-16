// edition:2024
//! Syntax matrix: let-chains.
//!
//! `flat_let_chains` holds two `if let ... && let ...` chains with flat bodies.
//! `let_chain_with_nested_if` holds two let-chains whose bodies nest an `if`.

/// Hold two let-chains with flat bodies.
fn flat_let_chains(a: Option<i32>, b: Option<i32>) -> i32 {
    let mut total = 0;
    if let Some(left) = a
        && let Some(right) = b
    {
        total += left + right;
    }
    if let Some(left) = b
        && let Some(right) = a
    {
        total -= left + right;
    }
    total
}

/// Hold two let-chains whose bodies nest an `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn let_chain_with_nested_if(a: Option<i32>, b: Option<i32>) -> i32 {
    let mut total = 0;
    if let Some(left) = a
        && let Some(right) = b
    {
        if left > right {
            total += left;
        }
    }
    if let Some(left) = b
        && let Some(right) = a
    {
        if left > right {
            total -= left;
        }
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (
        flat_let_chains(Some(1), Some(2)),
        let_chain_with_nested_if(Some(1), Some(2)),
    );
}
