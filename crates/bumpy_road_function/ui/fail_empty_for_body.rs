//! An empty `for` body is still claimed, and its iterable still charged.
//!
//! `for` lowering emits the authored block as the `Some` arm of an inner
//! `Iterator::next` match, and puts that match in the loop block's statement
//! list. A loop written with an empty body lowers exactly the same way: the
//! inner match is still the block's statement, and the authored arm is still
//! its non-`break` arm. So `extract_for_loop_body` succeeds here rather than
//! falling back, which is what the recognizer should do — the loop is still a
//! `for` loop the author wrote.
//!
//! The iterable below carries an authored cluster, and a second separated
//! cluster follows the loop. Both must be reported: claiming the shape must
//! still walk the scrutinee and record nothing for the empty body.

/// Count a bounded range into nothing, then adjust a total separately.
// @codescene(disable:"Bumpy Road Ahead", disable:"Complex Method", disable:"Complex Conditional") issue #504 fixture: the iterable cluster and second cluster are the lint input (2026-11-16)
fn count_then_adjust(first: i32, second: i32) -> i32 {
    for _value in (if first > 0
        && first < 100
        && first != 5
        && first != 7
        && first != 9
        && first != 11
    {
        0..first
    } else {
        0..0
    }) {}

    let mut total = 10;

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
    let _ = count_then_adjust(3, 1002);
}
