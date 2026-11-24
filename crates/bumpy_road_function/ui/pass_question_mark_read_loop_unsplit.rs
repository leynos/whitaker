//! The unsplit cuprum read loop, as measured under ADR 0016: no diagnostic.
//!
//! This is the shape of `consume_with_reader` in leynos/cuprum before
//! https://github.com/leynos/cuprum/pull/714 split it: a setup run that
//! propagates, then a loop whose body propagates on several lines and holds a
//! `break`. Each `?` is charged as an early-return branch (ADR 0016) on top of
//! the loop's nesting depth, and the result stays under the threshold because
//! the `?` lines are not adjacent. The fixture records that measured outcome;
//! it is not tuned to reach a verdict.
//!
//! CodeScene reports `Bumpy Road Ahead` with `bumps = 2` for this fixture
//! (CLI 1.0.36, 2026-11-16), but the two bumps are its two
//! `loop { if … { break } }` regions, not the `?` operators; the real cuprum
//! function has one such loop and CodeScene scores it 10.0. Whitaker's
//! silence here is the structural-region gap recorded in RFC 0013, not a `?`
//! question. The
//! `consume_in_helpers` function in `pass_question_mark_read_loop_split.rs` is
//! the same behaviour after the split and stays silent.
//!
//! The fixture is minimized: the tracing span, the retry counters and the UTF-8
//! decoder are replaced by stubs, because only the control flow is the lint
//! input.

use std::convert::TryFrom;

/// Failure modes of the stream pump.
#[derive(Debug)]
enum PumpError {
    BufferRangeExceeded,
    LengthOverflow,
}

/// Allocate the read buffer.
fn allocate(size: usize) -> Result<Vec<u8>, PumpError> {
    Ok(vec![0_u8; size])
}

/// Drain `read` to EOF and return the number of bytes seen.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: the unsplit loop is the lint input
fn consume_with_reader(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, PumpError>,
    size: usize,
) -> Result<u64, PumpError> {
    let mut buffer = allocate(size)?;
    let mut pending: Vec<u8> = Vec::new();
    let mut total_read = 0_u64;

    loop {
        let read_len = read(&mut buffer)?;
        if read_len == 0 {
            break;
        }
        let read_bytes = u64::try_from(read_len).map_err(|_| PumpError::LengthOverflow)?;
        total_read = total_read.saturating_add(read_bytes);
        let chunk = buffer
            .get(..read_len)
            .ok_or(PumpError::BufferRangeExceeded)?;
        pending.extend_from_slice(chunk);
    }

    let mut tail = allocate(size)?;
    let mut again = 0_u64;
    loop {
        let read_len = read(&mut tail)?;
        if read_len == 0 {
            break;
        }
        let read_bytes = u64::try_from(read_len).map_err(|_| PumpError::LengthOverflow)?;
        again = again.saturating_add(read_bytes);
        let chunk = tail.get(..read_len).ok_or(PumpError::BufferRangeExceeded)?;
        pending.extend_from_slice(chunk);
    }

    Ok(total_read + again)
}

fn main() {
    let _ = consume_with_reader(|_| Ok(0), 8);
}
