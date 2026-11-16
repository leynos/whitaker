//! Loops the desugaring handlers must decline, without inventing a diagnostic.
//!
//! `visit_for_loop` recognizes exactly one lowered shape and returns `false`
//! for anything else, leaving the ordinary match walk to score it. Returning
//! `false` is the safe default: it restores the pre-fix behaviour for that
//! expression instead of dropping structure the author did write.
//!
//! The loops below are the shapes that reach that fallback. None of them is
//! the `for`/`Iterator::next` lowering, so each is scored by the general walk.
//! All are flat, so the correct outcome is silence. A handler that claimed a
//! shape it does not own — or a fallback that recorded a segment while
//! declining — would surface here as a diagnostic.

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

/// Count down with a plain `loop`, the other `LoopSource` this handler declines.
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
