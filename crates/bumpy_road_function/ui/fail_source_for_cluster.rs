//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster sits inside a source `for` loop body, whose lowered
//! structure previously hid it from the detector.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

/// Accumulates values across two separated conditional clusters.
///
/// The first cluster is nested in a source `for` loop; the second follows
/// linear work.
// @codescene(disable:"Bumpy Road Ahead", disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the two separated clusters are the lint input (2026-11-12)
fn accumulate(limit: i32) -> i32 {
    let mut total = 0;

    for value in 0..limit {
        if value > 0
            && value < 100
            && value != 5
            && value != 7
            && value != 9
            && value != 11
        {
            if value % 2 == 0 {
                total += 1;
            }
            total += 2;
        }
    }

    total += limit;

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