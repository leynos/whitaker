//! Tests for fragment resolution, node counting and the stable pair digests.

use proptest::prelude::*;
use rstest::rstest;

use super::*;
use crate::{Fingerprint, FragmentId, FragmentLocation, NormProfile, Run0Error, ast::AstHash};

/// Build a fragment with the given identity, location and fingerprint hashes.
fn fragment(
    id: &str,
    file_uri: &str,
    span: std::ops::Range<usize>,
    hashes: &[u64],
) -> TokenFragment {
    TokenFragment::new(
        FragmentId::from(id),
        NormProfile::T1,
        FragmentLocation::new(file_uri, span),
        "source text\n",
    )
    .with_retained_fingerprints(
        hashes
            .iter()
            .map(|hash| Fingerprint::new(*hash, 0..1))
            .collect(),
    )
}

/// Build an accepted pair over two identifiers.
fn accepted(left: &str, right: &str, rule: CloneRule) -> AcceptedPair {
    let pair = CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .expect("distinct identifiers");
    let profile = match rule {
        CloneRule::Type1 => NormProfile::T1,
        CloneRule::Type2 | CloneRule::Type3 => NormProfile::T2,
    };
    AcceptedPair::new(pair, profile, crate::SimilarityRatio::new(1, 1))
}

proptest! {
    /// Check the digest byte-order helper against the standard converter.
    #[test]
    fn big_endian_bytes_match_to_be_bytes(value in any::<u64>()) {
        // `to_be_bytes` is the independent oracle. Rebuilding the bytes with
        // the production loop would let a shared mistake pass, and `token_hash`
        // depends on this order.
        #[expect(
            clippy::big_endian_bytes,
            reason = "pinning the digest's big-endian contract needs the standard converter"
        )]
        let expected = value.to_be_bytes();
        prop_assert_eq!(u64_big_endian_bytes(value), expected);
    }

    /// Check that the token digest ignores fingerprint order and duplicates.
    #[test]
    fn token_hash_is_order_and_duplicate_insensitive(
        left in proptest::collection::vec(any::<u64>(), 1..8),
        right in proptest::collection::vec(any::<u64>(), 1..8),
    ) {
        let forward = token_hash(
            &fragment("a", "src/a.rs", 0..4, &left),
            &fragment("b", "src/b.rs", 0..4, &right),
        );

        let mut reversed_left = left.clone();
        reversed_left.reverse();
        // Duplicating an entry must not change a set-shaped digest.
        if let Some(first) = reversed_left.first().copied() {
            reversed_left.push(first);
        }
        let mut reversed_right = right.clone();
        reversed_right.reverse();
        if let Some(first) = reversed_right.first().copied() {
            reversed_right.push(first);
        }
        let rebuilt = token_hash(
            &fragment("a", "src/a.rs", 0..4, &reversed_left),
            &fragment("b", "src/b.rs", 0..4, &reversed_right),
        );

        prop_assert_eq!(forward, rebuilt);
    }
}

/// Check that the pair digest distinguishes the rule it reports.
#[rstest]
#[case(CloneRule::Type1, CloneRule::Type2)]
#[case(CloneRule::Type2, CloneRule::Type3)]
fn pair_fingerprint_depends_on_the_rule(#[case] first: CloneRule, #[case] second: CloneRule) {
    let accepted_pair = accepted("alpha", "beta", first);

    assert_ne!(
        pair_fingerprint(&accepted_pair, first),
        pair_fingerprint(&accepted_pair, second),
        "the fragment digest must change with the reported rule"
    );
}

/// Check that reclassifying a pair keeps its digest when the rule is held
/// fixed, so a carried result keeps its Run 0 identity.
#[test]
fn pair_fingerprint_is_stable_for_the_same_rule() {
    let first = accepted("alpha", "beta", CloneRule::Type2);
    let second = accepted("alpha", "beta", CloneRule::Type2);

    assert_eq!(
        pair_fingerprint(&first, CloneRule::Type2),
        pair_fingerprint(&second, CloneRule::Type2),
        "identical pairs must produce identical digests"
    );
}

/// Check that the AST digest is independent of the reported rule and profile.
#[test]
fn ast_fingerprint_ignores_rule_and_profile() {
    let t1_left = fragment("a_t1", "src/a.rs", 0..10, &[1]);
    let t1_right = fragment("b_t1", "src/b.rs", 5..15, &[2]);
    let t2_left = TokenFragment::new(
        FragmentId::from("a_t2"),
        NormProfile::T2,
        FragmentLocation::new("src/a.rs", 0..10),
        "source text\n",
    );
    let t2_right = TokenFragment::new(
        FragmentId::from("b_t2"),
        NormProfile::T2,
        FragmentLocation::new("src/b.rs", 5..15),
        "source text\n",
    );

    assert_eq!(
        ast_pair_fingerprint(&t1_left, &t1_right),
        ast_pair_fingerprint(&t2_left, &t2_right),
        "physical identity must not depend on the normalization profile"
    );
}

/// Check that the AST digest distinguishes different physical pairs.
#[test]
fn ast_fingerprint_distinguishes_locations() {
    let left = fragment("a", "src/a.rs", 0..10, &[1]);
    let right = fragment("b", "src/b.rs", 5..15, &[2]);
    let moved = fragment("b", "src/b.rs", 6..15, &[2]);

    assert_ne!(
        ast_pair_fingerprint(&left, &right),
        ast_pair_fingerprint(&left, &moved),
        "a different span must produce a different AST digest"
    );
}

/// Check that the `astHash` value is the lowercase 64-hex-digit digest of both
/// canonical hashes in primary-then-related order.
#[test]
fn ast_hash_is_a_hex_digest_of_both_sides() {
    let primary = AstHash::from_hex("0123456789abcdef");
    let peer = AstHash::from_hex("fedcba9876543210");
    let digest = ast_hash(primary, peer);

    assert_eq!(
        digest.len(),
        64,
        "the AST digest must be 64 hex digits, got {digest}"
    );
    assert!(
        digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()),
        "the AST digest must be hexadecimal, got {digest}"
    );
    assert_ne!(
        digest,
        ast_hash(peer, primary),
        "the digest must respect primary-then-related order"
    );
}

/// Check that the canonical pair digest carries a 64-hex-digit shape.
#[test]
fn pair_fingerprints_are_64_hex_digits() {
    let accepted_pair = accepted("alpha", "beta", CloneRule::Type2);
    let digest = pair_fingerprint(&accepted_pair, CloneRule::Type2);

    assert_eq!(digest.len(), 64, "digests must be 64 hex digits");
    assert!(
        digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()),
        "digests must be hexadecimal"
    );
}

/// Check that a missing fragment is reported by identifier.
#[test]
fn missing_fragment_is_reported() {
    let fragments = vec![fragment("alpha", "src/a.rs", 0..4, &[1])];
    let map = fragment_map(&fragments);
    let pair = CandidatePair::new(FragmentId::from("alpha"), FragmentId::from("gamma"))
        .expect("distinct identifiers");

    match resolve_pair(&map, &pair) {
        Err(Run0Error::MissingFragment { fragment_id }) => assert_eq!(
            fragment_id, "gamma",
            "the error must name the absent fragment"
        ),
        other => panic!("expected a missing-fragment error, got {other:?}"),
    }
}

/// Check that an empty retained set is rejected by both guards.
#[test]
fn empty_fragments_are_rejected() {
    let empty = fragment("alpha", "src/a.rs", 0..4, &[]);

    assert!(
        matches!(
            ensure_non_empty(&empty),
            Err(Run0Error::EmptyFingerprintSet { .. })
        ),
        "an empty fragment must be rejected"
    );
    assert!(
        matches!(
            primary_fingerprint(&empty),
            Err(Run0Error::EmptyFingerprintSet { .. })
        ),
        "an empty fragment has no primary fingerprint"
    );
}

/// Check that the node count sums every depth bucket of a real tree.
#[test]
fn node_count_sums_all_buckets() {
    use crate::ast::{ByteSpan, KindId, NormalizedNode, NormalizedTree, kind_counts};

    let leaf = NormalizedNode::new(
        KindId::new(2),
        Some(crate::ast::LeafClass::Ident),
        Vec::new(),
    );
    let root = NormalizedNode::new(KindId::new(1), None, vec![leaf.clone(), leaf]);
    let Ok(span) = ByteSpan::new("fn f() {}", 0, 2) else {
        panic!("the fixture span should be valid");
    };
    let tree = NormalizedTree::new(root, span, span);
    let counts: KindCounts = kind_counts(&tree);

    assert_eq!(
        node_count(&counts),
        3,
        "one root and two leaves must sum to three nodes"
    );
}
