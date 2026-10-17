//! Bounds-checked numeric conversions for conditional-branch diagnostics.

/// Clamp diagnostic numbers to Fluent's signed 64-bit argument range.
pub(super) fn saturating_diagnostic_integer(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Convert a production diagnostic argument to Fluent's signed integer range.
pub(super) fn diagnostic_integer_argument(value: usize) -> i64 {
    saturating_diagnostic_integer(u64::try_from(value).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    //! Verify diagnostic integer conversion at Fluent's signed limit.

    use super::{diagnostic_integer_argument, saturating_diagnostic_integer};

    /// Verify values beyond Fluent's signed limit clamp without wrapping.
    #[test]
    fn diagnostic_integers_saturate_at_the_signed_boundary() {
        let maximum = i64::MAX.unsigned_abs();
        assert_eq!(saturating_diagnostic_integer(maximum - 1), i64::MAX - 1);
        assert_eq!(saturating_diagnostic_integer(maximum), i64::MAX);
        assert_eq!(saturating_diagnostic_integer(maximum + 1), i64::MAX);
    }

    /// Verify the production argument conversion clamps an oversized limit.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn production_diagnostic_arguments_saturate_at_the_signed_boundary() {
        let oversized = usize::try_from(u64::try_from(i64::MAX).expect("i64 max fits u64") + 1)
            .expect("oversized diagnostic limit fits usize");
        assert_eq!(diagnostic_integer_argument(oversized), i64::MAX);
    }
}
