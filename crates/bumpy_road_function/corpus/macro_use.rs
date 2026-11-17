//! Syntax matrix: macros.
//!
//! `nested_via_macro` invokes a local macro twice; each expansion contains a
//! nested `if`, but the authored function body holds only the invocations, so
//! Whitaker reports no region for generated code. `nested_in_macro_argument`
//! writes the nested `if` expressions inside `vec!` arguments, where the
//! source is authored.

/// Expand a nested `if` that the caller never wrote out.
macro_rules! clamp_pair {
    ($a:expr, $b:expr) => {
        if $a > 0 {
            if $b > 0 {
                $a + $b
            } else {
                $a
            }
        } else {
            0
        }
    };
}

/// Invoke a nested-`if` macro twice.
fn nested_via_macro(a: i32, b: i32) -> i32 {
    let first = clamp_pair!(a, b);
    let second = clamp_pair!(b, a);
    first + second
}

/// Write two nested `if` expressions in macro arguments.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_in_macro_argument(a: i32, b: i32) -> Vec<i32> {
    let first = vec![if a > 0 {
        if b > 0 { a + b } else { a }
    } else {
        0
    }];
    let second = vec![if b > 0 {
        if a > 0 { a - b } else { b }
    } else {
        0
    }];
    [first, second].concat()
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (nested_via_macro(1, 2), nested_in_macro_argument(1, 2));
}
