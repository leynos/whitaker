//! Lowercase hexadecimal rendering for digest bytes.
//!
//! `sha2` 0.11 returns `hybrid_array::Array<u8, _>` from `finalize`
//! and `digest`, and that type does not implement
//! [`core::fmt::LowerHex`]. This helper renders the raw digest bytes
//! without pulling in a dedicated hex-encoding dependency.

/// Encode `bytes` as a lowercase hexadecimal string.
///
/// Every byte is rendered as exactly two digits, including leading
/// zeroes, so the returned string is always twice the length of
/// `bytes`.
#[must_use]
pub(crate) fn to_lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_leading_zeroes_with_two_digits() {
        assert_eq!(to_lower_hex(&[0x00, 0x0f, 0xff, 0xa0]), "000fffa0");
    }

    #[test]
    fn empty_input_yields_empty_string() {
        assert_eq!(to_lower_hex(&[]), "");
    }
}
