//! UI fixture: a source `while` with a three-conjunct condition.
//!
//! The loop lowers to an `if`, and the condition is authored, so the lint must
//! charge it through the lowered form.
#![warn(conditional_max_n_branches)]

// @codescene(disable:"Complex Conditional") issue #504 fixture: the three-conjunct lowered condition is the lint input (2026-11-12)
fn source_while(mut a: bool, b: bool, c: bool) {
    while a && b && c { a = false; }
}

fn main() { source_while(true, true, true); }
