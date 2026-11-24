//! Fragment resolution and stable pair digests shared by both emitters.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::super::{
    AcceptedPair,
    CloneRule,
    Fingerprint,
    KindCounts,
    Run0Error,
    Run0Result,
    TokenFragment,
    ast::AstHash,
};
use crate::CandidatePair;

/// Index fragments by their stable identifier, in deterministic key order.
pub(crate) fn fragment_map(fragments: &[TokenFragment]) -> BTreeMap<&str, &TokenFragment> {
    fragments
        .iter()
        .map(|fragment| (fragment.id().as_str(), fragment))
        .collect()
}

/// Resolve both fragments of a pair, reporting the first missing identifier.
///
/// # Errors
///
/// Returns [`Run0Error::MissingFragment`] when either side is absent.
pub(crate) fn resolve_pair<'a>(
    fragment_map: &'a BTreeMap<&str, &'a TokenFragment>,
    pair: &CandidatePair,
) -> Run0Result<(&'a TokenFragment, &'a TokenFragment)> {
    let find = |id: &str| {
        fragment_map
            .get(id)
            .copied()
            .ok_or_else(|| Run0Error::MissingFragment {
                fragment_id: id.to_owned(),
            })
    };
    Ok((find(pair.left().as_str())?, find(pair.right().as_str())?))
}

/// Require a fragment to retain at least one fingerprint.
///
/// # Errors
///
/// Returns [`Run0Error::EmptyFingerprintSet`] for an empty retained set.
pub(crate) fn ensure_non_empty(fragment: &TokenFragment) -> Run0Result<()> {
    if fragment.retained_fingerprints().is_empty() {
        return Err(Run0Error::EmptyFingerprintSet {
            fragment_id: fragment.id().as_str().to_owned(),
        });
    }
    Ok(())
}

/// Return a fragment's primary fingerprint.
///
/// # Errors
///
/// Returns [`Run0Error::EmptyFingerprintSet`] when the retained set is empty.
pub(crate) fn primary_fingerprint(fragment: &TokenFragment) -> Run0Result<&Fingerprint> {
    fragment
        .retained_fingerprints()
        .first()
        .ok_or_else(|| Run0Error::EmptyFingerprintSet {
            fragment_id: fragment.id().as_str().to_owned(),
        })
}

/// Digest an accepted pair's identity and rule into its SARIF fragment key.
pub(crate) fn pair_fingerprint(accepted: &AcceptedPair, rule: CloneRule) -> String {
    let mut hasher = Sha256::new();
    hasher.update(accepted.pair().left().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(accepted.pair().right().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(rule.label().as_bytes());
    digest_hex(hasher.finalize())
}

/// Digest an AST-pass pair's physical identity.
///
/// The identity is the two fragments' `(uri, start, end)` triples in canonical
/// order plus the literal `"AST"`, so it survives a change of normalization
/// profile or of reported rule. Both sides must already be in canonical order.
pub(crate) fn ast_pair_fingerprint(left: &TokenFragment, right: &TokenFragment) -> String {
    let mut hasher = Sha256::new();
    for fragment in [left, right] {
        hasher.update(fragment.file_uri().as_bytes());
        hasher.update([0]);
        hasher.update(fragment.span().start.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(fragment.span().end.to_string().as_bytes());
        hasher.update([0]);
    }
    hasher.update(b"AST");
    digest_hex(hasher.finalize())
}

/// Digest the two canonical AST hashes of a pair into its `astHash` value.
///
/// The composition is `left NUL right` in the result's primary-then-related
/// order, mirroring how [`token_hash`] composes the token digests.
pub(crate) fn ast_hash(primary: AstHash, peer: AstHash) -> String {
    let mut hasher = Sha256::new();
    hasher.update(primary.to_hex().as_bytes());
    hasher.update([0]);
    hasher.update(peer.to_hex().as_bytes());
    digest_hex(hasher.finalize())
}

/// Format the two fragments' token fingerprints into a stable digest.
pub(crate) fn token_hash(left: &TokenFragment, right: &TokenFragment) -> String {
    let mut values = left
        .retained_fingerprints()
        .iter()
        .chain(right.retained_fingerprints().iter())
        .map(|fingerprint| fingerprint.hash)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();

    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(u64_big_endian_bytes(value));
    }
    digest_hex(hasher.finalize())
}

/// Serializes a `u64` as big-endian bytes with explicit masks and shifts.
///
/// The digest contract fixes the byte order, so the decomposition is spelled
/// out here rather than delegated to an endianness-specific method.
fn u64_big_endian_bytes(value: u64) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    let mut remaining = value;
    for slot in bytes.iter_mut().rev() {
        *slot = (remaining & 0xff) as u8;
        remaining >>= 8;
    }
    bytes
}

/// Render bytes as lowercase hexadecimal.
fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::new();
    for byte in bytes.as_ref() {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

/// Render one hexadecimal digit.
fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0'.saturating_add(value)),
        _ => char::from(b'a'.saturating_add(value.saturating_sub(10))),
    }
}

/// Total node count of a fragment's feature counts, used by the `min_nodes`
/// guard.
pub(crate) fn node_count(counts: &KindCounts) -> usize {
    counts.iter().map(|(_, _, count)| count as usize).sum()
}

#[cfg(test)]
#[path = "fingerprint_tests.rs"]
mod tests;
