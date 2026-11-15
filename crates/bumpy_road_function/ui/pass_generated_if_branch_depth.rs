//! UI fixture that must stay silent.
//!
//! The `when!` macro writes the `if` and takes its branch bodies as authored
//! block arguments. A generated `if` contributes no nesting depth of its own,
//! so the authored `if` inside the then-branch adds only its own charge.
//! Charging depth for the generated `if` would lift those lines over the
//! threshold and report a cluster that the authored code does not contain.

/// A macro-supplied `if` around authored branch bodies.
macro_rules! when {
    ($cond:expr, $then:block, $else:block) => {
        if $cond $then else $else
    };
}

/// Classifies a value with one guarded cluster and one plain cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the generated `if` and its authored branches are the lint input (2026-11-15)
fn classify(value: Option<i32>, fallback: i32) -> i32 {
    let mut total = fallback;

    if value.is_some()
        && when!(
            value.is_none(),
            {
                if total > 0
                    && total < 100
                {
                    true
                } else {
                    false
                }
            },
            { false }
        )
    {
        total += 1;
    }

    if total > 1000
        && total < 2000
        && total != 1500
        && total != 1750
        && total != 1800
        && total != 1900
    {
        total += 2;
    }

    total
}

fn main() { classify(None, 0); }