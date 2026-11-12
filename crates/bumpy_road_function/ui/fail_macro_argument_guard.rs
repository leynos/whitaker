//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster is a multi-conjunct match guard supplied as a macro
//! argument, which the generated `matches!` wrapper previously hid.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

/// Classifies a value, then applies a second conditional cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the guard and second cluster are the lint input (2026-11-12)
fn classify(value: Option<i32>, fallback: i32) -> i32 {
    let limit = 11;
    let is_special = matches!(
        value,
        Some(inner)
            if inner > 0
                && inner < 100
                && inner != 5
                && inner != 7
                && inner != 9
                && inner != limit
    );

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

fn dead_code_fixture_marker() {}

fn main() {}