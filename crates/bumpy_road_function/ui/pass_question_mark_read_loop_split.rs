//! The helper-split read loop must stay silent.
//!
//! This is `pass_question_mark_read_loop_unsplit.rs` after the refactor that
//! https://github.com/leynos/cuprum/pull/714 applied to `consume_with_reader`:
//! the per-read work moves into named helpers with declared inputs and outputs,
//! so the loop tells a story and no single function carries clustered
//! propagation and nesting. ADR 0016 charges each `?` as an early-return branch; this shape
//! spreads those branches across helpers and is the refactor the lint exists to
//! foster.

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

/// Read once, reporting `None` at end of stream.
fn read_nonempty(
    read: &mut impl FnMut(&mut [u8]) -> Result<usize, PumpError>,
    buffer: &mut [u8],
) -> Result<Option<usize>, PumpError> {
    let read_len = read(buffer)?;
    Ok((read_len != 0).then_some(read_len))
}

/// Convert a read length to the running total's type.
fn read_length_as_u64(read_len: usize) -> Result<u64, PumpError> {
    u64::try_from(read_len).map_err(|_| PumpError::LengthOverflow)
}

/// Return the filled prefix of `buffer`.
fn filled_prefix(buffer: &[u8], read_len: usize) -> Result<&[u8], PumpError> {
    buffer
        .get(..read_len)
        .ok_or(PumpError::BufferRangeExceeded)
}

/// Fold one read into `pending` and return the bytes it contributed.
fn absorb_read(
    buffer: &[u8],
    read_len: usize,
    pending: &mut Vec<u8>,
) -> Result<u64, PumpError> {
    let bytes = read_length_as_u64(read_len)?;
    pending.extend_from_slice(filled_prefix(buffer, read_len)?);
    Ok(bytes)
}

/// Drain `read` to end of stream and return the number of bytes seen.
fn drain(
    read: &mut impl FnMut(&mut [u8]) -> Result<usize, PumpError>,
    size: usize,
    pending: &mut Vec<u8>,
) -> Result<u64, PumpError> {
    let mut buffer = allocate(size)?;
    let mut total = 0_u64;
    while let Some(read_len) = read_nonempty(read, &mut buffer)? {
        total = total.saturating_add(absorb_read(&buffer, read_len, pending)?);
    }
    Ok(total)
}

/// Drain `read` twice and return the combined byte count.
fn consume_in_helpers(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, PumpError>,
    size: usize,
) -> Result<u64, PumpError> {
    let mut pending: Vec<u8> = Vec::new();
    let first = drain(&mut read, size, &mut pending)?;
    let second = drain(&mut read, size, &mut pending)?;
    Ok(first + second)
}

fn main() {
    let _ = consume_in_helpers(|_| Ok(0), 8);
}
