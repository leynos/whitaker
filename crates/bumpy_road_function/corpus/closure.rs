//! Syntax matrix: closures.
//!
//! `closures_with_nested_regions` has no nested conditional of its own; both
//! nested regions sit inside closure bodies. Whitaker keeps the closure
//! boundary, so the enclosing function reports no region (the closures are
//! analysed on their own only when `include_closures` is set).
//! `nested_regions_outside_closure` is the same logic written inline.

/// Hold two nested regions, each inside a closure body.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: CodeScene charges the closure regions here (2026-11-16)
fn closures_with_nested_regions(a: i32, b: i32) -> i32 {
    let first = |value: i32| {
        let mut total = 0;
        if value > 0 {
            if b > 0 {
                total += 1;
            }
        }
        total
    };
    let second = |value: i32| {
        let mut total = 0;
        if value < 0 {
            if a < 0 {
                total -= 1;
            }
        }
        total
    };
    first(a) + second(b)
}

/// Hold the same two nested regions inline.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_regions_outside_closure(a: i32, b: i32) -> i32 {
    let mut total = 0;
    if a > 0 {
        if b > 0 {
            total += 1;
        }
    }
    if b < 0 {
        if a < 0 {
            total -= 1;
        }
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (
        closures_with_nested_regions(1, 2),
        nested_regions_outside_closure(1, 2),
    );
}
