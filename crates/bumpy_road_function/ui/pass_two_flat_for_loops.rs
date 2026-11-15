//! Two separated, lexically flat `for` loops must produce no diagnostic.
//!
//! rustc lowers a `for` loop into an outer `match` over `IntoIterator`, a
//! `loop`, and an inner `match` over `Iterator::next`. Desugarings are
//! transparent to the provenance classifier, so the generic walk charged every
//! lowered node: a flat `for` loop accumulated three nesting depths and two
//! control-flow weights over its own lines. Two such loops therefore crossed
//! the detection threshold even though neither contains a conditional, and the
//! diagnostic claimed "clusters of nested conditional logic" where the author
//! wrote none.
//!
//! Both loops below are flat: no nesting, no branching, no guards.

/// Return the prefix sums of `signal` followed by its running maxima.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: flat loops must stay silent
fn prefix_then_maxima(signal: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut prefix = Vec::with_capacity(signal.len() + 1);
    prefix.push(0.0_f64);
    for &value in signal {
        let next = prefix.last().copied().unwrap_or(0.0) + value;
        prefix.push(next);
    }

    let mut maxima = Vec::with_capacity(signal.len());
    for &value in signal {
        let highest = maxima.last().copied().unwrap_or(value);
        maxima.push(value.max(highest));
    }

    (prefix, maxima)
}

fn main() { let _ = prefix_then_maxima(&[1.0, 2.0, 3.0]); }
