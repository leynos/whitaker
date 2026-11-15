//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster is a multi-conjunct guard inside an authored `while`
//! condition. The loop is lowered to an `if`, and that condition is charged
//! once over its logical spine, which counts the `matches!` call as a single
//! branch and stops there, so the guard is measurable only when the traversal
//! descends below that charge.

/// Drains a value, then applies a second conditional cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the loop-condition guard and second cluster are the lint input (2026-11-15)
fn drain(value: Option<i32>, start: i32) -> i32 {
    let limit = 11;
    let mut total = start;

    while matches!(
        value,
        Some(inner)
            if inner > 0
                && inner < 100
                && inner != 5
                && inner != 7
                && inner != 9
                && inner != limit
    ) && total < 1000
    {
        total += 1;
    }

    if total > 2000
        && total < 3000
        && total != 2500
        && total != 2750
        && total != 2800
        && total != 2900
    {
        total -= 1;
    }

    total
}

fn main() { drain(None, 0); }