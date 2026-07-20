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
    //! Tests for the lowercase hexadecimal digest formatter.

    use super::*;

    #[test]
    fn renders_leading_zeroes_with_two_digits() {
        assert_eq!(to_lower_hex(&[0x00, 0x0f, 0xff, 0xa0]), "000fffa0");
    }

    #[test]
    fn empty_input_yields_empty_string() {
        assert_eq!(to_lower_hex(&[]), "");
    }

    #[test]
    fn every_byte_renders_as_two_lowercase_round_tripping_digits() {
        // Bounded exhaustive check over the whole u8 range: each byte must
        // render as exactly two lowercase ASCII hex digits that parse back to
        // the original value.
        for byte in u8::MIN..=u8::MAX {
            let rendered = to_lower_hex(&[byte]);
            assert_eq!(rendered.len(), 2, "byte {byte:#04x} must render two digits");
            assert!(
                rendered
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "byte {byte:#04x} rendered non-lowercase-hex output {rendered:?}",
            );
            let parsed = u8::from_str_radix(&rendered, 16).expect("two hex digits parse as a byte");
            assert_eq!(parsed, byte, "round-trip mismatch for byte {byte:#04x}");
        }
    }
}
