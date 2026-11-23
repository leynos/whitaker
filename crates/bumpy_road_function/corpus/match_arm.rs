//! Syntax matrix: conditionals inside `match` arms.
//!
//! `nested_match_arm` holds two separate matches whose arms each contain an
//! `if`. `flat_match` holds two matches whose arms are flat expressions.
//! `one_match_two_arm_ifs` holds a single match with an `if` in each of two
//! arms, which is the RFC's open question about grouping within one match.

/// Hold two separate matches each with a nested `if`.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two regions are the input (2026-11-16)
fn nested_match_arm(a: Option<i32>, b: Option<i32>) -> i32 {
    let mut total = 0;
    match a {
        Some(value) => {
            if value > 0 {
                total += value;
            }
        }
        None => {}
    }
    match b {
        Some(value) => {
            if value > 0 {
                total -= value;
            }
        }
        None => {}
    }
    total
}

/// Hold two matches whose arms are flat.
fn flat_match(a: Option<i32>, b: Option<i32>) -> i32 {
    let first = match a {
        Some(value) => value,
        None => 0,
    };
    let second = match b {
        Some(value) => -value,
        None => 0,
    };
    first + second
}

/// Hold one match with an `if` in each of two arms.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: grouping within one match is measured (2026-11-16)
fn one_match_two_arm_ifs(a: Result<i32, i32>) -> i32 {
    match a {
        Ok(value) => {
            if value > 10 {
                return value;
            }
            0
        }
        Err(value) => {
            if value > 10 {
                return -value;
            }
            1
        }
    }
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let _ = (
        nested_match_arm(Some(1), None),
        flat_match(Some(1), None),
        one_match_two_arm_ifs(Ok(1)),
    );
}
