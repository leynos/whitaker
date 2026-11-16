//! Loops the desugaring handlers must decline, without inventing a diagnostic.
//!
//! `visit_for_loop` recognizes exactly one lowered shape and returns `false`
//! for anything else, leaving the ordinary match walk to score it. Returning
//! `false` is the safe default: it restores the pre-fix behaviour for that
//! expression instead of dropping structure the author did write.
//!
//! Measurement decides which loop takes which path, and it splits three ways.
//! The `for` loops are lowerings `visit_for_loop` owns, and a probe shows
//! `extract_for_loop_body` reads both, so they are claimed. The `while let`
//! body is claimed too: rustc lowers `while` and `while let` through the same
//! routine and stamps both `LoopSource::While`, so `handle_while_loop` owns it.
//! Only `count_down`'s plain `loop` is declined, because its source is
//! `LoopSource::Loop`, and the ordinary block walk scores it. Two claimed
//! shapes and one declined shape, all silent.
//!
//! Every loop here is flat, so silence is correct whichever path scores it. A
//! handler that invented a bump for a shape it merely claimed would fail here,
//! so the fixture rejects the over-eager direction. It is not a tight guard on
//! the declining direction: a declined `for` lowering falls through to the
//! ordinary arm walk, which charges the scaffolding the recognizer exists to
//! remove. That fallback is pinned instead by `fail_source_for_cluster`, whose
//! byte-exact expected output shifts as soon as `visit_for_loop` declines a
//! lowering it owns.

/// Stop early on the first negative sample.
fn stop_at_negative(signal: &[i32]) -> i32 {
    let mut total = 0;
    for &value in signal {
        if value < 0 {
            break;
        }
        total += value;
    }
    total
}

/// Skip negative samples and keep accumulating.
fn skip_negative(signal: &[i32]) -> i32 {
    let mut total = 0;
    for &value in signal {
        if value < 0 {
            continue;
        }
        total += value;
    }
    total
}

/// Drain a fallible iterator with `while let`, which is not a `for` lowering.
fn drain_while_let(values: &mut std::vec::IntoIter<i32>) -> i32 {
    let mut total = 0;
    while let Some(value) = values.next() {
        total += value;
    }
    total
}

/// Count down with a plain `loop`, the other shape this handler declines.
fn count_down(mut remaining: i32) -> i32 {
    let mut steps = 0;
    loop {
        if remaining <= 0 {
            break;
        }
        steps += 1;
        remaining -= 1;
    }
    steps
}

fn main() {
    let _ = stop_at_negative(&[1, 2, -1, 3]);
    let _ = skip_negative(&[1, -2, 3]);
    let mut values = vec![1, 2, 3].into_iter();
    let _ = drain_while_let(&mut values);
    let _ = count_down(4);
}
