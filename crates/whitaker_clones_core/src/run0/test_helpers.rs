//! Shared test helpers for Run 0 acceptance and emission tests.

use crate::{CandidatePair, Fingerprint, FragmentId, NormProfile};

use super::{TokenFragment, TokenPassConfig};

pub(super) struct FragmentInput<'a> {
    pub(super) id: &'a str,
    pub(super) profile: NormProfile,
    pub(super) file_uri: &'a str,
    pub(super) source_text: &'a str,
    pub(super) hashes: &'a [(u64, std::ops::Range<usize>)],
}

pub(super) fn fingerprint(hash: u64, range: std::ops::Range<usize>) -> Fingerprint {
    Fingerprint::new(hash, range)
}

pub(super) fn fragment(input: FragmentInput<'_>) -> TokenFragment {
    TokenFragment::new(
        FragmentId::from(input.id),
        input.profile,
        input.file_uri,
        input.source_text,
    )
    .with_retained_fingerprints(
        input
            .hashes
            .iter()
            .map(|(hash, range)| fingerprint(*hash, range.clone()))
            .collect(),
    )
}

pub(super) fn pair(left: &str, right: &str) -> CandidatePair {
    CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .unwrap_or_else(|| panic!("pair `{left}` and `{right}` must be distinct"))
}

pub(super) fn config() -> TokenPassConfig {
    TokenPassConfig::new("whitaker_clones_cli@token", env!("CARGO_PKG_VERSION"))
}
