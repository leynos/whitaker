//! The two-loop read function with every `?` removed (the control).
//!
//! `two_loops_with_try.rs` propagates with `?` throughout. This twin has the
//! same two `loop { ... if cond { break } ... }` blocks with infallible
//! stand-ins, so any CodeScene finding comes from the loops and not from `?`.
//! CodeScene reports `bumps = 2` for both, so `?` adds nothing to its regions.

use std::convert::TryFrom;

/// Allocate the read buffer.
fn allocate(size: usize) -> Vec<u8> {
    vec![0_u8; size]
}

/// Drain `read` to EOF and return the number of bytes seen.
// @codescene(disable:"Bumpy Road Ahead") corpus fixture: the two loop regions are the input (2026-11-16)
fn consume_with_reader(mut read: impl FnMut(&mut [u8]) -> usize, size: usize) -> u64 {
    let mut buffer = allocate(size);
    let mut pending: Vec<u8> = Vec::new();
    let mut total_read = 0_u64;

    loop {
        let read_len = read(&mut buffer);
        if read_len == 0 {
            break;
        }
        let read_bytes = u64::try_from(read_len).unwrap_or(u64::MAX);
        total_read = total_read.saturating_add(read_bytes);
        let chunk = buffer.get(..read_len).unwrap_or(&[]);
        pending.extend_from_slice(chunk);
    }

    let mut tail = allocate(size);
    let mut again = 0_u64;
    loop {
        let read_len = read(&mut tail);
        if read_len == 0 {
            break;
        }
        let read_bytes = u64::try_from(read_len).unwrap_or(u64::MAX);
        again = again.saturating_add(read_bytes);
        let chunk = tail.get(..read_len).unwrap_or(&[]);
        pending.extend_from_slice(chunk);
    }

    total_read + again
}

fn main() {
    let _ = consume_with_reader(|_| 0, 8);
}
