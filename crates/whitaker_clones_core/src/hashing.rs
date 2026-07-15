//! Shared deterministic FNV-style hashing helpers.
//!
//! Clone detection depends on hashes that are stable across builds, machines,
//! and supported CPU architectures. This module owns the byte-mixing primitives
//! used by token fingerprints and canonical AST subtree hashes so both paths
//! share one deterministic overflow and serialization contract.
//!
//! Callers must normalize their inputs before mixing them here. Integer helpers
//! use little-endian bytes explicitly, and all arithmetic is wrapping FNV-style
//! multiplication so equivalent source produces equivalent hashes on every
//! supported platform. AST hashes also mix [`PARSER_SCHEMA_VERSION`] so parser
//! updates cannot silently compare incompatible tree shapes.

pub(crate) const RABIN_KARP_BASE: u64 = 1_000_003;
pub(crate) const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
// Both branches intentionally define the same value. Tests expose the prime so
// historical rolling-hash vectors can verify it directly, while production keeps
// it private so callers depend on the stable mix helpers instead.
#[cfg(test)]
pub(crate) const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
#[cfg(not(test))]
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Parser schema version mixed into AST hashes.
pub const PARSER_SCHEMA_VERSION: &str =
    concat!("ra_ap_syntax=", env!("WHITAKER_RA_AP_SYNTAX_VERSION"));

/// Mixes one already-normalized byte into the crate's stable FNV-style stream.
///
/// All token and AST hashes go through this primitive so the byte folding rule
/// is shared. The function is intentionally tiny and wrapping: stability across
/// platforms matters more here than detecting arithmetic overflow.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(mix_byte(FNV_OFFSET_BASIS, b'a'), 0xaf63_bd4c_8601_b7be);
/// ```
pub(crate) fn mix_byte(current: u64, byte: u8) -> u64 {
    current.wrapping_mul(FNV_PRIME) ^ u64::from(byte)
}

/// Mixes a byte slice by repeatedly applying [`mix_byte`].
///
/// Callers are responsible for serializing higher-level values into canonical
/// bytes before calling this helper. Keeping the loop here avoids each feature
/// extractor inventing its own traversal order or overflow behaviour.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(mix_bytes(FNV_OFFSET_BASIS, b"ab"), 0x0832_6707_b4eb_37b8);
/// ```
pub(crate) fn mix_bytes(mut current: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        current = mix_byte(current, *byte);
    }
    current
}

/// Mixes a `u16` using little-endian bytes.
///
/// AST kind identifiers are lowered as opaque `u16` values. Little-endian
/// serialization makes that representation explicit so hashes do not depend on
/// the host CPU's byte order.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(mix_u16(FNV_OFFSET_BASIS, 0x1234), 0x0832_9407_b4eb_8443);
/// ```
pub(crate) fn mix_u16(current: u64, value: u16) -> u64 {
    mix_bytes(current, &value.to_le_bytes())
}

/// Mixes a `u64` using little-endian bytes.
///
/// Merkle-style AST hashes fold child counts and child hashes through this
/// helper. The fixed byte order is the cross-platform contract; callers choose
/// where a numeric boundary belongs in the stream.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(
///     mix_u64(FNV_OFFSET_BASIS, 0x0102_0304_0506_0708),
///     0x999a_7071_7b39_65dd
/// );
/// ```
pub(crate) fn mix_u64(current: u64, value: u64) -> u64 {
    mix_bytes(current, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{FNV_OFFSET_BASIS, mix_byte, mix_bytes, mix_u16, mix_u64};

    #[rstest]
    #[case::single_byte(mix_byte(FNV_OFFSET_BASIS, b'a'), 0xaf63_bd4c_8601_b7be)]
    #[case::byte_slice(mix_bytes(FNV_OFFSET_BASIS, b"ab"), 0x0832_6707_b4eb_37b8)]
    #[case::u16_little_endian(mix_u16(FNV_OFFSET_BASIS, 0x1234), 0x0832_9407_b4eb_8443)]
    #[case::u64_little_endian(
        mix_u64(FNV_OFFSET_BASIS, 0x0102_0304_0506_0708),
        0x999a_7071_7b39_65dd
    )]
    fn mix_functions_match_fixed_vectors(#[case] actual: u64, #[case] expected: u64) {
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::byte(|| mix_byte(FNV_OFFSET_BASIS, b'z'))]
    #[case::bytes(|| mix_bytes(FNV_OFFSET_BASIS, b"stable"))]
    #[case::u16_value(|| mix_u16(FNV_OFFSET_BASIS, 0xbeef))]
    #[case::u64_value(|| mix_u64(FNV_OFFSET_BASIS, 0xfeed_face_cafe_babe))]
    fn mix_functions_are_deterministic(#[case] hash: fn() -> u64) {
        assert_eq!(hash(), hash());
    }

    #[rstest]
    fn integer_mixers_use_little_endian_bytes() {
        assert_eq!(
            mix_u16(FNV_OFFSET_BASIS, 0x1234),
            mix_bytes(FNV_OFFSET_BASIS, &[0x34, 0x12])
        );
        assert_eq!(
            mix_u64(FNV_OFFSET_BASIS, 0x0102_0304_0506_0708),
            mix_bytes(
                FNV_OFFSET_BASIS,
                &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
            )
        );
    }
}
