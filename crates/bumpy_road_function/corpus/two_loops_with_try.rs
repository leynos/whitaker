//! Two read loops, each with `loop { ... if cond { break } ... }`, and `?`
//! throughout. CodeScene reports `bumps = 2`: each loop plus its `if` is one
//! region. `two_loops_no_try.rs` is the same function with every `?` removed.
//!
//! This is the shape of `consume_with_reader` in leynos/cuprum before
//! https://github.com/leynos/cuprum/pull/714 split it, and
//! `two_loops_split.rs` is the same behaviour after the split.
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
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two loop regions are the input (2026-11-16)
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
