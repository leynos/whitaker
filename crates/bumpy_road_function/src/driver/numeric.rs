//! Bounds-checked numeric conversions shared by the bumpy-road diagnostics.

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

#[cfg(test)]
mod tests {
    //! Verify conversion behaviour at diagnostic and compiler integer limits.

    use super::{saturating_branch_count, saturating_diagnostic_integer, span_byte_offset};

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
}
