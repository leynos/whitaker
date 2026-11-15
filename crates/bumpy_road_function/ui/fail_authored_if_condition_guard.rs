//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster is a multi-conjunct guard inside an authored `if`
//! condition. The condition is charged once over its logical spine, which
//! counts the `matches!` call as a single branch and stops there, so the guard
//! is measurable only when the traversal descends below that charge.

/// Classifies a value, then applies a second conditional cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the condition guard and second cluster are the lint input (2026-11-15)
fn classify(value: Option<i32>, fallback: i32) -> i32 {
    let limit = 11;
    let mut total = fallback;

    if matches!(
        value,
        Some(inner)
            if inner > 0
                && inner < 100
                && inner != 5
                && inner != 7
                && inner != 9
                && inner != limit
    ) && value.is_some()
    {
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