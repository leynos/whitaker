//! UI fixture that should trigger the bumpy road lint.
//!
//! The first cluster is a nested `match` in the tail of a block guard supplied
//! as a macro argument. The generated `matches!` match charges the guard's own
//! predicate, which counts the tail as a single branch and stops there, so the
//! arms are measurable only when the generated wrapper descends below that
//! predicate.

/// Classifies a value, then applies a second conditional cluster.
// @codescene(disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the guard-tail and second clusters are the lint input (2026-11-14)
fn classify(value: Option<i32>, fallback: i32) -> i32 {
    let is_special = matches!(
        value,
        Some(inner) if {
            match inner {
                0 => true,
                positive if positive > 0 => {
                    if positive < 100 {
                        if positive % 2 == 0 {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
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

fn main() { classify(None, 0); }