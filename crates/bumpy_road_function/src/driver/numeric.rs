//! Bounds-checked numeric conversions shared by the bumpy-road diagnostics.

use std::ops::RangeInclusive;

/// Clamp a diagnostic number to the largest value supported by Fluent.
pub(super) fn saturating_diagnostic_integer(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Clamp a branch count to the largest value representable by segment weights.
pub(super) fn saturating_branch_count(branches: u64) -> u32 {
    u32::try_from(branches).unwrap_or(u32::MAX)
}

/// Convert a snippet offset to the compiler's `u32` range, rejecting larger
/// values so the caller can omit an unrepresentable source span.
pub(super) fn span_byte_offset(offset: u64) -> Option<u32> {
    u32::try_from(offset).ok()
}

/// Convert a production diagnostic argument to Fluent's signed integer range.
pub(super) fn diagnostic_integer_argument(value: usize) -> i64 {
    saturating_diagnostic_integer(u64::try_from(value).unwrap_or(u64::MAX))
}

/// Compute the production predicate weight with a bounded branch count.
pub(super) fn predicate_segment_weight(branches: usize, weight: f64) -> f64 {
    f64::from(saturating_branch_count(
        u64::try_from(branches).unwrap_or(u64::MAX),
    )) * weight
}

/// Convert production snippet offsets to compiler byte positions.
pub(super) fn span_byte_offset_range(start: usize, end: usize) -> Option<(u32, u32)> {
    let start = u64::try_from(start).ok()?;
    let end = u64::try_from(end).ok()?;
    Some((span_byte_offset(start)?, span_byte_offset(end)?))
}

/// Map an inclusive line range to checked compiler byte offsets.
pub(super) fn line_range_byte_offsets(
    line_starts: &[usize],
    snippet_len: usize,
    base_line: usize,
    lines: RangeInclusive<usize>,
) -> Option<(u32, u32)> {
    let start_line = *lines.start();
    let end_line = *lines.end();
    if start_line < base_line || end_line < start_line {
        return None;
    }

    let start_index = start_line - base_line;
    let end_index = end_line - base_line;
    let start_offset = *line_starts.get(start_index)?;
    let end_offset = line_starts
        .get(end_index + 1)
        .copied()
        .unwrap_or(snippet_len);

    span_byte_offset_range(start_offset, end_offset)
}

#[cfg(test)]
mod tests {
    //! Verify conversion behaviour at diagnostic and compiler integer limits.

    use super::{
        diagnostic_integer_argument, line_range_byte_offsets, predicate_segment_weight,
        saturating_branch_count, saturating_diagnostic_integer, span_byte_offset,
        span_byte_offset_range,
    };

    /// Verify values beyond Fluent's signed limit clamp without wrapping.
    #[test]
    fn diagnostic_integers_saturate_at_the_signed_boundary() {
        let maximum = i64::MAX.unsigned_abs();
        assert_eq!(saturating_diagnostic_integer(maximum - 1), i64::MAX - 1);
        assert_eq!(saturating_diagnostic_integer(maximum), i64::MAX);
        assert_eq!(saturating_diagnostic_integer(maximum + 1), i64::MAX);
    }

    /// Verify branch counts beyond the segment-weight limit clamp to `u32::MAX`.
    #[test]
    fn branch_counts_saturate_at_the_unsigned_boundary() {
        let maximum = u64::from(u32::MAX);
        assert_eq!(saturating_branch_count(maximum - 1), u32::MAX - 1);
        assert_eq!(saturating_branch_count(maximum), u32::MAX);
        assert_eq!(saturating_branch_count(maximum + 1), u32::MAX);
    }

    /// Verify compiler span offsets accept `u32::MAX` and reject larger values.
    #[test]
    fn span_offsets_reject_values_above_the_compiler_boundary() {
        let maximum = u64::from(u32::MAX);
        assert_eq!(span_byte_offset(maximum - 1), Some(u32::MAX - 1));
        assert_eq!(span_byte_offset(maximum), Some(u32::MAX));
        assert_eq!(span_byte_offset(maximum + 1), None);
    }

    /// Verify the diagnostic call-site conversion clamps an oversized count.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn production_diagnostic_arguments_saturate_at_the_signed_boundary() {
        let oversized = usize::try_from(u64::try_from(i64::MAX).expect("i64 max fits u64") + 1)
            .expect("oversized diagnostic count fits usize");
        assert_eq!(diagnostic_integer_argument(oversized), i64::MAX);
    }

    /// Verify the production segment-weight conversion clamps excess branches.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn production_predicate_weight_saturates_excess_branches() {
        let oversized =
            usize::try_from(u64::from(u32::MAX) + 1).expect("oversized branch count fits usize");
        assert_eq!(
            predicate_segment_weight(oversized, 0.5),
            f64::from(u32::MAX) * 0.5,
        );
    }

    /// Verify the production span-offset conversion rejects an unrepresentable end.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn production_span_offset_ranges_reject_an_unrepresentable_end() {
        let oversized =
            usize::try_from(u64::from(u32::MAX) + 1).expect("oversized span offset fits usize");
        assert_eq!(span_byte_offset_range(0, oversized), None);
    }

    /// Verify the production line-to-span mapping rejects unrepresentable ranges.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn production_line_range_mapping_rejects_an_unrepresentable_offset() {
        let oversized =
            usize::try_from(u64::from(u32::MAX) + 1).expect("oversized span offset fits usize");
        assert_eq!(
            line_range_byte_offsets(&[0, oversized], oversized, 1, 1..=1),
            None,
        );
    }
}
