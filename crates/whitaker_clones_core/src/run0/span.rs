//! Byte-range to SARIF-region conversion helpers.

use whitaker_sarif::{Region, RegionBuilder};

use super::error::{Run0Error, Run0Result};

pub(crate) fn region_for_range(
    fragment_id: &str,
    source_text: &str,
    range: std::ops::Range<usize>,
) -> Run0Result<Region> {
    validate_range(fragment_id, source_text, &range)?;
    let starts = line_starts(source_text);
    let (start_line, start_column) = line_and_column(source_text, &starts, range.start);
    // `validate_range` has already proved `range.end` is an in-bounds char
    // boundary, so the fallible slice only fails defensively.
    let Some(prefix) = source_text.get(..range.end) else {
        return Err(Run0Error::InvalidUtf8Boundary {
            fragment_id: fragment_id.to_owned(),
        });
    };
    let end_position = prefix
        .char_indices()
        .next_back()
        .map_or(range.start, |(i, _)| i);
    let (end_line, end_column) = line_and_column(source_text, &starts, end_position);

    RegionBuilder::new(start_line)
        .with_start_column(start_column)
        .with_end_line(end_line)
        .with_end_column(end_column)
        .with_byte_offset(range.start)
        .with_byte_length(range.end.saturating_sub(range.start))
        .build()
        .map_err(Run0Error::from)
}

fn validate_range(
    fragment_id: &str,
    source_text: &str,
    range: &std::ops::Range<usize>,
) -> Run0Result<()> {
    if range.start >= range.end || range.end > source_text.len() {
        return Err(Run0Error::InvalidFingerprintRange {
            fragment_id: fragment_id.to_owned(),
            start: range.start,
            end: range.end,
            source_len: source_text.len(),
        });
    }
    if !source_text.is_char_boundary(range.start) || !source_text.is_char_boundary(range.end) {
        return Err(Run0Error::InvalidUtf8Boundary {
            fragment_id: fragment_id.to_owned(),
        });
    }
    Ok(())
}

fn line_starts(source_text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source_text.char_indices() {
        if ch == '\n' {
            starts.push(index.saturating_add(ch.len_utf8()));
        }
    }
    starts
}

fn line_and_column(source_text: &str, starts: &[usize], offset: usize) -> (usize, usize) {
    let line_index = starts
        .partition_point(|start| *start <= offset)
        .saturating_sub(1);
    // `starts` always begins with 0, so both fallbacks preserve the
    // start-of-text behaviour if the partition point ever degenerated.
    let line_start = starts.get(line_index).copied().unwrap_or(0);
    let clamped_offset = offset.min(source_text.len());
    let line_slice = source_text.get(line_start..clamped_offset).unwrap_or("");
    let utf16_count = line_slice.encode_utf16().count();
    (line_index.saturating_add(1), utf16_count.saturating_add(1))
}
