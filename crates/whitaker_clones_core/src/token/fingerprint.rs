//! Shingling, Rabin-Karp rolling hashing, and winnowing helpers.

use super::types::{Fingerprint, NormalizedToken, ShingleSize, WinnowWindow};

pub(super) const RABIN_KARP_BASE: u64 = 1_000_003;
pub(super) const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
pub(super) const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Builds Rabin-Karp fingerprints for all `k`-sized normalized token windows.
///
/// If fewer than `k` tokens are available, the returned vector is empty.
///
/// # Examples
///
/// ```
/// use std::convert::TryFrom;
///
/// use whitaker_clones_core::{NormProfile, ShingleSize, hash_shingles, normalize};
///
/// let tokens = normalize("fn demo() { 1 + 2 }", NormProfile::T2)?;
/// let fingerprints = hash_shingles(&tokens, ShingleSize::try_from(3)?);
///
/// assert!(!fingerprints.is_empty());
/// # Ok::<(), whitaker_clones_core::TokenPassError>(())
/// ```
#[must_use]
pub fn hash_shingles(tokens: &[NormalizedToken], k: ShingleSize) -> Vec<Fingerprint> {
    let width = k.get();
    if tokens.len() < width {
        return Vec::new();
    }

    let codes = tokens.iter().map(stable_token_code).collect::<Vec<_>>();
    let mut hashes = Vec::with_capacity(tokens.len() - width + 1);
    let highest_power = highest_power(width);

    let mut rolling = 0_u64;
    for code in codes.iter().take(width) {
        rolling = rolling.wrapping_mul(RABIN_KARP_BASE).wrapping_add(*code);
    }
    #[expect(clippy::indexing_slicing, reason = "bounds pre-validated at line 29")]
    hashes.push(Fingerprint::new(
        rolling,
        tokens[0].range.start..tokens[width - 1].range.end,
    ));

    for start in 1..=(tokens.len() - width) {
        #[expect(
            clippy::indexing_slicing,
            reason = "bounds pre-validated by loop range"
        )]
        let outgoing = codes[start - 1];
        #[expect(
            clippy::indexing_slicing,
            reason = "bounds pre-validated by loop range"
        )]
        let incoming = codes[start + width - 1];
        rolling = rolling
            .wrapping_sub(outgoing.wrapping_mul(highest_power))
            .wrapping_mul(RABIN_KARP_BASE)
            .wrapping_add(incoming);
        #[expect(
            clippy::indexing_slicing,
            reason = "bounds pre-validated by loop range"
        )]
        hashes.push(Fingerprint::new(
            rolling,
            tokens[start].range.start..tokens[start + width - 1].range.end,
        ));
    }

    hashes
}

/// Applies deterministic winnowing using the rightmost minimum in each window.
///
/// When the fingerprint count is less than or equal to the window size, the
/// global rightmost minimum is retained once.
///
/// # Examples
///
/// ```
/// use std::convert::TryFrom;
///
/// use whitaker_clones_core::{Fingerprint, WinnowWindow, winnow};
///
/// let fingerprints = vec![
///     Fingerprint::new(7, 0..1),
///     Fingerprint::new(3, 1..2),
///     Fingerprint::new(5, 2..3),
/// ];
///
/// let retained = winnow(&fingerprints, WinnowWindow::try_from(4)?);
/// assert_eq!(retained, vec![Fingerprint::new(3, 1..2)]);
/// # Ok::<(), whitaker_clones_core::TokenPassError>(())
/// ```
#[must_use]
pub fn winnow(fingerprints: &[Fingerprint], window: WinnowWindow) -> Vec<Fingerprint> {
    if fingerprints.is_empty() {
        return Vec::new();
    }

    let width = window.get();
    if fingerprints.len() <= width {
        #[expect(clippy::indexing_slicing, reason = "bounds pre-checked at line 86")]
        return vec![fingerprints[rightmost_minimum_index(fingerprints)].clone()];
    }

    let mut retained = Vec::new();
    let mut last_index = None;

    for start in 0..=(fingerprints.len() - width) {
        let end = start + width;
        #[expect(
            clippy::indexing_slicing,
            reason = "bounds pre-validated by loop range"
        )]
        let index = start + rightmost_minimum_index(&fingerprints[start..end]);
        if last_index != Some(index) {
            #[expect(
                clippy::indexing_slicing,
                reason = "index derived from valid window slice"
            )]
            retained.push(fingerprints[index].clone());
            last_index = Some(index);
        }
    }

    retained
}

fn highest_power(width: usize) -> u64 {
    let mut power = 1_u64;
    for _ in 1..width {
        power = power.wrapping_mul(RABIN_KARP_BASE);
    }
    power
}

fn stable_token_code(token: &NormalizedToken) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;

    hash = hash_byte(hash, token_kind_tag(token));
    // Formatting through `Display` keeps the token-code scheme aligned with the
    // public labels; profile before replacing this with a more bespoke path.
    for byte in token.kind.to_string().bytes() {
        hash = hash_byte(hash, byte);
    }

    hash
}

fn token_kind_tag(token: &NormalizedToken) -> u8 {
    match token.kind {
        super::types::NormalizedTokenKind::Atom(_) => b'a',
        super::types::NormalizedTokenKind::Identifier(_) => b'i',
        super::types::NormalizedTokenKind::Lifetime(_) => b'l',
        super::types::NormalizedTokenKind::Literal(_) => b'v',
    }
}

fn hash_byte(current: u64, byte: u8) -> u64 {
    current.wrapping_mul(FNV_PRIME) ^ u64::from(byte)
}

/// Returns the index of the rightmost minimum hash in the window.
///
/// # Panics
///
/// The `window` slice must be non-empty. Calling this function with an empty
/// slice will cause out-of-bounds access.
fn rightmost_minimum_index(window: &[Fingerprint]) -> usize {
    debug_assert!(!window.is_empty(), "window must be non-empty");
    let mut best_index = 0_usize;

    for (index, fingerprint) in window.iter().enumerate().skip(1) {
        #[expect(clippy::indexing_slicing, reason = "best_index always < window.len()")]
        let best = &window[best_index];
        if fingerprint.hash <= best.hash {
            best_index = index;
        }
    }

    best_index
}
