//! Syntax matrix: match-arm guards.
//!
//! `flat_guards` holds two matches whose arms carry `if` guards and flat
//! bodies; guards are not nested regions. `guard_with_nested_if` holds two
//! matches whose guarded arms also nest an `if` in the body.

/// Hold two matches with guarded arms and flat bodies.
fn flat_guards(a: i32, b: i32) -> i32 {
    let first = match a {
        value if value > 10 => 1,
        value if value > 5 => 2,
        _ => 3,
    };
    let second = match b {
        value if value > 10 => 4,
        value if value > 5 => 5,
        _ => 6,
    };
    first + second
}

/// Hold two matches whose guarded arms nest an `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn guard_with_nested_if(a: i32, b: i32) -> i32 {
    let mut total = 0;
    match a {
        value if value > 10 => {
            if b > 0 {
                total += 1;
            }
        }
        _ => {}
    }
    match b {
        value if value > 10 => {
            if a > 0 {
                total += 2;
            }
        }
        _ => {}
    }
    total
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (flat_guards(1, 2), guard_with_nested_if(1, 2));
}
