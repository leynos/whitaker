//! A `?` inside a comment must not be taken for the operator.
//!
//! The charge sits on the line of the `?` token. Each statement below puts a
//! comment that ends in a question mark on the line before the real `?`, so a
//! search that stopped at the first `?` in the text after the operand would
//! charge the comment line instead. The adjacent configuration gives depth a
//! large weight and the others none, so that the bump boundaries in the
//! expected output move with the line the charge lands on.

/// Run a fallible step.
fn step(value: u32) -> Result<u32, String> {
    if value > 1000 {
        Err(String::from("too large"))
    } else {
        Ok(value + 1)
    }
}

/// Chain eight steps in two separated runs of four.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: the two `?` runs are the lint input
fn two_runs(start: u32) -> Result<u32, String> {
    let a = step(start) // is this fine?
        ?;
    let b = step(a) // and this?
        ?;
    let c = step(b) // and this?
        ?;
    let d = step(c) /* still fine?
        */ ?;

    let gap = d + 1;
    let wider_gap = gap + 1;
    let widest_gap = wider_gap + 1;

    let e = step(widest_gap) // is this fine?
        ?;
    let f = step(e) // and this?
        ?;
    let g = step(f) // and this?
        ?;
    let h = step(g) /* still fine?
        */ ?;

    Ok(h)
}

fn main() {
    let _ = two_runs(1);
}
