//! Byte-range to SARIF-region conversion shared by both emitters.
//!
//! SARIF columns default to UTF-16 code units, and this module is the single
//! place that converts a byte offset into that coordinate system, so both runs
//! report identical regions for identical fragments.

use std::ops::Range;

use whitaker_sarif::{Region, RegionBuilder};

use super::super::run0::{Run0Error, Run0Result};

/// Convert a source byte range into a one-based SARIF region.
///
/// # Errors
///
/// Returns [`Run0Error::InvalidFingerprintRange`] for an empty, inverted or
/// out-of-bounds range, and [`Run0Error::InvalidUtf8Boundary`] when either
/// offset splits a UTF-8 character.
pub(crate) fn region_for_range(
    fragment_id: &str,
    source_text: &str,
    range: Range<usize>,
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
        .map_or(range.start, |(index, _)| index);
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

/// Format a source region as a compact line and column span.
pub(crate) fn compact_span(region: &Region) -> String {
    format!(
        "{}:{}-{}:{}",
        region.start_line,
        region.start_column.unwrap_or(1),
        region.end_line.unwrap_or(region.start_line),
        region
            .end_column
            .unwrap_or_else(|| region.start_column.unwrap_or(1))
    )
}

/// Reject ranges that cannot describe a non-empty slice of the source.
fn validate_range(fragment_id: &str, source_text: &str, range: &Range<usize>) -> Run0Result<()> {
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

/// Collect the byte offset of every line start.
fn line_starts(source_text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source_text.char_indices() {
        if ch == '\n' {
            starts.push(index.saturating_add(ch.len_utf8()));
        }
    }
    starts
}

/// Map a byte offset to one-based line and UTF-16 column coordinates.
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

#[cfg(test)]
mod tests {
    //! Region conversion tests, including multi-byte and multi-line cases.

    use super::*;

    /// Check that a single-line range maps to one-based coordinates.
    #[test]
    fn single_line_range_maps_to_line_and_column() {
        let region = region_for_range("alpha", "fn a() {}\n", 3..4).expect("valid range");

        assert_eq!(
            (
                region.start_line,
                region.start_column,
                region.end_line,
                region.end_column
            ),
            (1, Some(4), Some(1), Some(4)),
            "the region must use one-based line and column coordinates"
        );
    }

    /// Check that columns count UTF-16 code units, as SARIF defaults require.
    #[test]
    fn columns_count_utf16_code_units() {
        // The emoji is one scalar value but two UTF-16 code units, and it
        // occupies four bytes starting after the nine-byte prefix.
        let region = region_for_range("alpha", "let x = '😀';", 9..13).expect("valid range");

        assert_eq!(
            region.start_column,
            Some(10),
            "the column after a nine-unit prefix must be ten"
        );
        assert_eq!(
            region.end_column,
            Some(10),
            "a single scalar value is one column wide"
        );
    }

    /// Check that an empty or inverted range is rejected.
    #[test]
    fn empty_range_is_rejected() {
        let result = region_for_range("alpha", "fn a() {}", 4..4);

        assert!(
            matches!(result, Err(Run0Error::InvalidFingerprintRange { .. })),
            "an empty range must be rejected, got {result:?}"
        );
    }

    /// Check that a range past the source length is rejected.
    #[test]
    fn out_of_bounds_range_is_rejected() {
        let result = region_for_range("alpha", "fn a() {}", 0..99);

        assert!(
            matches!(result, Err(Run0Error::InvalidFingerprintRange { .. })),
            "an out-of-bounds range must be rejected, got {result:?}"
        );
    }

    /// Check that a range splitting a UTF-8 character is rejected.
    #[test]
    fn non_char_boundary_range_is_rejected() {
        // '😀' occupies bytes 0..4, so 0..2 splits it.
        let result = region_for_range("alpha", "😀", 0..2);

        assert!(
            matches!(result, Err(Run0Error::InvalidUtf8Boundary { .. })),
            "a split character must be rejected, got {result:?}"
        );
    }

    /// Check the compact span rendering, including its defaulting rules.
    #[test]
    fn compact_span_renders_defaults() {
        let built = RegionBuilder::new(4).with_start_column(2).build();
        let Ok(compact) = built else {
            panic!("the fixture region should build");
        };

        assert_eq!(
            compact_span(&compact),
            "4:2-4:2",
            "an absent end must fall back to the start coordinates"
        );
    }
}
