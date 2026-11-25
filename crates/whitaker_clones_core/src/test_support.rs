//! Shared test helpers for token-pass and AST-pass tests.
//!
//! Run 0 tests use the fingerprint-shaped fixtures; Run 1 tests also need
//! explicit physical spans and synthetic lowered trees, which live here so
//! both suites share one definition.

use crate::{
    CandidatePair,
    Fingerprint,
    FragmentId,
    FragmentLocation,
    NormProfile,
    TokenFragment,
    TokenPassConfig,
};

pub(crate) struct FragmentInput<'a> {
    pub(crate) id: &'a str,
    pub(crate) profile: NormProfile,
    pub(crate) file_uri: &'a str,
    pub(crate) source_text: &'a str,
    pub(crate) hashes: &'a [(u64, std::ops::Range<usize>)],
}

/// Derive a fragment span from the hull of its retained fingerprint ranges.
///
/// Run 0 never reads the fragment span, so deriving it keeps the token-pass
/// fixtures focused on fingerprints. Run 1 tests, which do read the span,
/// construct their locations explicitly.
fn hull_span(hashes: &[(u64, std::ops::Range<usize>)]) -> std::ops::Range<usize> {
    let lowest = hashes.iter().map(|(_, range)| range.start).min();
    let highest = hashes.iter().map(|(_, range)| range.end).max();
    match (lowest, highest) {
        (Some(begin), Some(finish)) => begin..finish,
        _ => 0..0,
    }
}

pub(crate) fn fingerprint(hash: u64, range: std::ops::Range<usize>) -> Fingerprint {
    Fingerprint::new(hash, range)
}

/// Build a token fragment and attach each retained fingerprint range.
pub(crate) fn fragment(input: &FragmentInput<'_>) -> TokenFragment {
    TokenFragment::new(
        FragmentId::from(input.id),
        input.profile,
        FragmentLocation::new(input.file_uri, hull_span(input.hashes)),
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

/// Builds a canonical candidate pair, yielding `None` for a degenerate self-pair.
pub(crate) fn pair(left: &str, right: &str) -> Option<CandidatePair> {
    CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
}

pub(crate) fn config() -> TokenPassConfig {
    TokenPassConfig::new("whitaker_clones_cli@token", env!("CARGO_PKG_VERSION"))
}

/// Source text long enough for every run-1 fixture span.
///
/// The bytes are printable ASCII so byte offsets, character boundaries and
/// SARIF columns all agree, which keeps region assertions readable.
pub(crate) const FIXTURE_SOURCE: &str = "fn alpha() {\n    let value = compute(1, 2);\n    value \
                                         + other(value)\n}\nfn beta() {\n    let value = \
                                         compute(3, 4);\n    value + other(value)\n}\nfn gamma() \
                                         {\n    let value = compute(5, 6);\n    value - \
                                         other(value)\n}\nfn delta() {\n    let value = \
                                         compute(7, 8);\n    value * other(value)\n}\n";

/// Build a validated fixture span.
///
/// # Panics
///
/// Panics when the offsets do not describe `FIXTURE_SOURCE`, which indicates a
/// defect in the test fixture rather than a recoverable condition.
pub(crate) fn fixture_span(start: u32, end: u32) -> crate::ByteSpan {
    match crate::ByteSpan::new(FIXTURE_SOURCE, start, end) {
        Ok(span) => span,
        Err(error) => panic!("invalid fixture span {start}..{end}: {error}"),
    }
}

/// Build a synthetic normalized tree with a chosen node count and kind tag.
///
/// The tag varies every node kind so two trees with different tags hash and
/// score differently, while two trees with the same tag differ only in size.
/// The shape is a root over two branches, each carrying one interior node and
/// a share of the leaves, so the tree has depth three and therefore real
/// trigram productions: `(root, branch, interior)` twice and
/// `(branch, interior, leaf)` once per leaf.
///
/// # Panics
///
/// Panics when `nodes` is fewer than five, which cannot carry a trigram, or
/// when the span is not a valid slice of `FIXTURE_SOURCE`.
pub(crate) fn synthetic_tree(
    kind_tag: u16,
    nodes: usize,
    span: crate::ByteSpan,
) -> crate::NormalizedTree {
    use crate::ast::{KindId, LeafClass, NormalizedNode, NormalizedTree};

    const FIXED_NODES: usize = 5;
    assert!(
        nodes >= FIXED_NODES,
        "a trigram-carrying tree needs at least {FIXED_NODES} nodes"
    );
    let branch_kind = KindId::new(kind_tag.saturating_add(2));
    let interior_kind = KindId::new(kind_tag.saturating_add(3));
    let leaf_kind = KindId::new(kind_tag.saturating_add(4));
    // The root, two branches and two interiors are the fixed nodes, so the
    // leaves carry the remainder. Getting this wrong makes every size request
    // off by one against the `min_nodes` guard.
    let leaves = nodes.saturating_sub(FIXED_NODES);
    // Splitting the leaves between the two branches keeps the shape symmetric
    // without integer division: the second branch takes the remainder.
    let half = leaves.div_euclid(2);

    let branch = |count: usize| {
        let leaf = NormalizedNode::new(leaf_kind, Some(LeafClass::Ident), Vec::new());
        let interior = NormalizedNode::new(
            interior_kind,
            None,
            std::iter::repeat_n(leaf, count).collect(),
        );
        NormalizedNode::new(branch_kind, None, vec![interior])
    };
    let children = vec![branch(half), branch(leaves - half)];
    let root = NormalizedNode::new(KindId::new(kind_tag.saturating_add(1)), None, children);
    NormalizedTree::new(root, span, span)
}

/// Identity and location of a fixture fragment.
pub(crate) struct FragmentSpec<'a> {
    /// Fixture name, also used as the fragment identifier.
    pub(crate) id: &'a str,
    /// Normalization profile the fragment was produced under.
    pub(crate) profile: NormProfile,
    /// Source artefact URI.
    pub(crate) file_uri: &'a str,
    /// Half-open byte span of the fragment.
    pub(crate) span: std::ops::Range<usize>,
    /// Retained fingerprint hashes.
    pub(crate) hashes: &'a [u64],
}

/// Build a token fragment over `FIXTURE_SOURCE` from its specification.
pub(crate) fn fixture_fragment(spec: &FragmentSpec<'_>) -> TokenFragment {
    TokenFragment::new(
        FragmentId::from(spec.id),
        spec.profile,
        crate::FragmentLocation::new(spec.file_uri, spec.span.clone()),
        FIXTURE_SOURCE,
    )
    .with_retained_fingerprints(
        spec.hashes
            .iter()
            .map(|hash| Fingerprint::new(*hash, 0..1))
            .collect(),
    )
}
