//! Bounds-checked numeric conversions for conditional-branch diagnostics.

/// Clamp diagnostic numbers to Fluent's signed 64-bit argument range.
pub(super) fn saturating_diagnostic_integer(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::saturating_diagnostic_integer;

    #[test]
    fn diagnostic_integers_saturate_at_the_signed_boundary() {
        let maximum = i64::MAX.unsigned_abs();
        assert_eq!(saturating_diagnostic_integer(maximum - 1), i64::MAX - 1);
        assert_eq!(saturating_diagnostic_integer(maximum), i64::MAX);
        assert_eq!(saturating_diagnostic_integer(maximum + 1), i64::MAX);
    }
}
