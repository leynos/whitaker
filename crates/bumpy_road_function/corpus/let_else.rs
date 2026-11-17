//! Syntax matrix: `let ... else`.
//!
//! `flat_let_else` holds two `let ... else` guards that return early; each is
//! a guard, not a nested region. `let_else_with_nested_if` holds two
//! `let ... else` guards whose `else` blocks nest an `if` before diverging.

/// Hold two `let ... else` guards with flat `else` blocks.
fn flat_let_else(a: Option<i32>, b: Option<i32>) -> i32 {
    let Some(left) = a else {
        return -1;
    };
    let Some(right) = b else {
        return -2;
    };
    left + right
}

/// Hold two `let ... else` guards whose `else` blocks nest an `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn let_else_with_nested_if(a: Option<i32>, b: Option<i32>) -> i32 {
    let Some(left) = a else {
        if b.is_some() {
            return -1;
        }
        return -2;
    };
    let Some(right) = b else {
        if left > 0 {
            return -3;
        }
        return -4;
    };
    left + right
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (
        flat_let_else(Some(1), Some(2)),
        let_else_with_nested_if(Some(1), Some(2)),
    );
}
