//! Adjacent `?` lines inside loops are charged on the `?`, not on a `)`.
//!
//! ADR 0016 charges each `?` as an early-return branch: predicate 0.5, depth
//! 1.0 and flow 0.5, which is 2.0 on the line of the `?` token. Two such lines
//! in a row inside a loop body (depth 1.0) smooth to a sustained peak above the
//! 2.5 threshold, so two separated pairs are two bumps. The pairs below are
//! parenthesized with the closing parenthesis on the next line: the expression
//! span then ends on a line that has no `?` in it, and a charge placed on the
//! last byte of the span lands on the wrong line and shifts or splits the bump.
//! The bump line ranges in the expected output pin the `?` lines.

/// Run a fallible step.
fn step(value: u32) -> Result<u32, String> {
    if value > 1000 {
        Err(String::from("too large"))
    } else {
        Ok(value + 1)
    }
}

/// Step twice per iteration, in two separated loops.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: the two `?` runs are the lint input
fn step_in_two_loops(start: u32) -> Result<u32, String> {
    let mut total = start;
    loop {
        total = (step(total)?
        ) + (step(total)?
        ) + (step(total)?
        ) + step(total)?;
        break;
    }

    let mut sum = 0;
    for value in 0..3 {
        sum += value;
    }
    let mut again = total + sum;

    loop {
        again = (step(again)?
        ) + (step(again)?
        ) + (step(again)?
        ) + step(again)?;
        break;
    }

    Ok(again)
}

fn main() {
    let _ = step_in_two_loops(1);
}
