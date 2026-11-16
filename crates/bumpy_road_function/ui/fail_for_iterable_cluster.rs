//! A `for` loop's authored iterable must still be charged.
//!
//! The lowered `for` is a `match` over `IntoIterator::into_iter(iter)`, and
//! `visit_for_loop` charges the construct the author wrote by walking that
//! scrutinee and then the authored body. The scaffolding between the two — the
//! outer arm, the loop, and the `Iterator::next` match — is skipped.
//!
//! That trade is only correct if the scrutinee walk actually happens. The
//! pre-provenance detector skipped the whole desugared `for` and so lost any
//! cluster written in the iterable. `fail_source_for_cluster` covers the body
//! half of this contract; its iterable is a plain range, so it cannot see the
//! scrutinee walk go missing. Here the first cluster is the iterable itself: an
//! implementation that claimed the lowering without walking the scrutinee
//! records no bump for it, leaving a single cluster and therefore no
//! diagnostic.

/// Sum a bounded range, then accumulate a second, separated cluster.
///
/// The first cluster is the iterable expression; the second follows the loop.
fn sum_then_adjust(first: i32, second: i32) -> i32 {
    let mut total = 0;

    for value in (if first > 0
        && first < 100
        && first != 5
        && first != 7
        && first != 9
        && first != 11
    {
        0..first
    } else {
        0..0
    }) {
        total += value;
    }

    total += 10;

    if second > 1000
        && second < 2000
        && second != 1500
        && second != 1750
        && second != 1800
        && second != 1900
    {
        if second % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    total
}

fn main() {
    let _ = sum_then_adjust(3, 1002);
}
