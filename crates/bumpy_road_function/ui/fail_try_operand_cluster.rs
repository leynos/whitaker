//! A `?` operand's authored cluster must still be charged.
//!
//! `?` is scored by walking its operand and charging nothing for the lowered
//! `match` over `Try::branch`, whose arms are the compiler's. That trade is
//! only correct if the operand itself is still visited: an implementation that
//! skipped the whole desugared match — the pre-provenance behaviour — would
//! silently drop any authored nesting written inside the operand.
//!
//! Each `?` below applies to a block that contains a nested conditional, so
//! the two separated clusters must still be detected. A `visit_try` that
//! returns without walking the scrutinee produces no diagnostic here, and this
//! fixture is what makes that regression visible.

/// Resolve both halves through `?`, accumulating two separated clusters.
fn resolve_both(first: i32, second: i32) -> Result<i32, String> {
    let low = {
        if first > 0
            && first < 100
            && first != 5
            && first != 7
            && first != 9
            && first != 11
        {
            if first % 2 == 0 {
                Ok(first + 1)
            } else {
                Ok(first + 2)
            }
        } else {
            Err(String::from("out of the low range"))
        }
    }?;

    let high = {
        if second > 1000
            && second < 2000
            && second != 1500
            && second != 1750
            && second != 1800
            && second != 1900
        {
            if second % 3 == 0 {
                Ok(second + 3)
            } else {
                Ok(second + 4)
            }
        } else {
            Err(String::from("out of the high range"))
        }
    }?;

    Ok(low + high)
}

fn main() {
    let _ = resolve_both(1, 2);
}
