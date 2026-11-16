//! An explicit `match` keeps the charge that `?` gives up.
//!
//! The `?` operator and an explicit `match` over the same fallible call are the
//! two ways to write one propagation. `visit_try` treats only the `?` form as
//! `MatchSource::TryDesugar` and declines everything else, so the explicit
//! `match` below takes the ordinary arm walk and is charged one nesting level
//! and one control-flow weight per arm.
//!
//! That asymmetry is the whole contract: the `?` arms are lowering the author
//! never wrote, while the `match` arms are the author's own structure. A
//! `visit_try` that claimed more than `TryDesugar` would score the explicit
//! form as scaffolding too, and the diagnostic below would disappear.
//!
//! Both functions call the same fallible operation and both carry a nested
//! conditional from their first cluster. The second cluster in each is
//! identical, so the only difference between them is how the propagation is
//! written.

/// Propagate through `?`, accumulating two separated clusters.
// @codescene(disable:"Bumpy Road Ahead", disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the operand cluster and second cluster are the lint input (2026-11-16)
fn through_question(first: i32, second: i32) -> Result<i32, String> {
    let low = divide(first, 2)?;

    let mut total = low + 10;

    if second > 1000
        && second < 2000
        && second != 1500
        && second != 1750
        && second != 1800
        && second != 1900
    {
        if second % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    Ok(total)
}

/// Propagate through an explicit `match`, keeping both authored arms.
// @codescene(disable:"Bumpy Road Ahead", disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the match arms and second cluster are the lint input (2026-11-16)
fn through_match(first: i32, second: i32) -> Result<i32, String> {
    let low = match divide(first, 2) {
        Ok(value) => value,
        Err(reason) => {
            if reason.len() > 3 && reason.len() < 40 && reason.len() != 17 {
                return Err(String::from("rejected after inspection"));
            }
            return Err(reason);
        }
    };

    let mut total = low + 10;

    if second > 1000
        && second < 2000
        && second != 1500
        && second != 1750
        && second != 1800
        && second != 1900
    {
        if second % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    Ok(total)
}

/// Divide, rejecting a zero divisor with a reason the caller may inspect.
fn divide(value: i32, divisor: i32) -> Result<i32, String> {
    if divisor == 0 {
        Err(String::from("cannot divide by zero"))
    } else {
        Ok(value / divisor)
    }
}

fn main() {
    let _ = through_question(4, 1002);
    let _ = through_match(4, 1002);
}
