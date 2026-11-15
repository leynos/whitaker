//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster is a guard whose `!` comes from a macro body and whose
//! operand is an authored `if` predicate. The guard's own span is generated, so
//! no predicate segment is recorded for it; the authored condition must still
//! be measured, which happens only when a guard that charged nothing is visited
//! as an ordinary expression rather than as a spine that was already charged.

/// A macro-supplied negation around an authored predicate.
macro_rules! negated {
    ($predicate:expr) => {
        !$predicate
    };
}

/// Classifies a value, then applies a second conditional cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the generated-guard cluster and second cluster are the lint input (2026-11-15)
fn classify(value: Option<i32>, fallback: i32) -> i32 {
    let limit = 11;
    let is_special = match value {
        Some(inner)
            if negated!(if inner > 0
                && inner < 100
                && inner != 5
                && inner != 7
                && inner != 9
                && inner != limit
            {
                false
            } else {
                true
            }) =>
        {
            true
        }
        _ => false,
    };

    let mut total = fallback;

    if is_special {
        total += 1;
    }

    total += 2;

    if total > 1000
        && total < 2000
        && total != 1500
        && total != 1750
        && total != 1800
        && total != 1900
    {
        if total % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    total
}

fn main() { classify(None, 0); }