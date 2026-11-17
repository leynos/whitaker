//! `?` carries no flow weight: only that weight is non-zero here, and it stays
//! silent.
//!
//! ADR 0016 charges a `?` exactly what the authored `if` with an early return
//! it abbreviates earns: one predicate branch and one nesting depth. An
//! authored `return` carries no flow weight in `SegmentBuilder`, so `?` carries
//! none either. The adjacent configuration sets the flow weight to 3.0 and the
//! other two to zero; the same eight-step function that flags in
//! `fail_question_mark_predicate_weight.rs` and
//! `fail_question_mark_depth_weight.rs` must therefore produce no diagnostic.

/// Run a fallible step.
fn step(value: u32) -> Result<u32, String> {
    if value > 1000 {
        Err(String::from("too large"))
    } else {
        Ok(value + 1)
    }
}

/// Chain eight steps in two separated runs of four.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: mirrors the sibling weight fixtures
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
