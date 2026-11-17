//! `?` is charged the depth weight: only that weight is non-zero here.
//!
//! ADR 0016 charges a `?` one predicate branch and one nesting depth on its
//! own line. The adjacent configuration sets the depth weight to 3.0 and the
//! other two to zero, so a bump can only appear if the depth weight is charged.
//! The sibling fixtures `fail_question_mark_predicate_weight.rs` and
//! `pass_question_mark_flow_weight_not_charged.rs` isolate the other weights.
//! Four adjacent `?` lines smooth to 3.0 in the middle, over the 2.5 threshold,
//! and two such runs are two bumps.

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
    let a = step(start)?;
    let b = step(a)?;
    let c = step(b)?;
    let d = step(c)?;

    let gap = d + 1;
    let wider_gap = gap + 1;
    let widest_gap = wider_gap + 1;

    let e = step(widest_gap)?;
    let f = step(e)?;
    let g = step(f)?;
    let h = step(g)?;

    Ok(h)
}

fn main() {
    let _ = two_runs(1);
}
