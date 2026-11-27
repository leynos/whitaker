//! Refinement tests: memoization, failure dispositions and both decision tables.

use std::collections::BTreeMap;

use super::{AstPassConfig, Refinement, RefinementInput, refine_with_lowering};
use crate::{
    AcceptedPair,
    AstError,
    AstResult,
    ByteSpan,
    CandidatePair,
    CloneRule,
    FragmentId,
    NormProfile,
    NormalizedTree,
    RefinedPair,
    Run1Error,
    SimilarityRatio,
    TokenFragment,
    TokenPassConfig,
    test_support::{FragmentSpec, fixture_fragment, fixture_span, synthetic_tree},
};

/// Node count that clears the default `min_nodes` guard.
pub(super) const NODES: usize = 20;

/// Token-pass configuration used by the refinement tests.
pub(super) fn token_config() -> TokenPassConfig {
    TokenPassConfig::new("whitaker_clones_cli@token", "0.3.0")
}

/// AST-pass configuration used by the refinement tests.
pub(super) fn ast_config() -> AstPassConfig { AstPassConfig::new("0.3.0") }

/// Build a fragment over the shared fixture source.
pub(super) fn fragment(
    id: &str,
    profile: NormProfile,
    span: std::ops::Range<usize>,
    hashes: &[u64],
) -> TokenFragment {
    fixture_fragment(&FragmentSpec {
        id,
        profile,
        file_uri: "src/fixture.rs",
        span,
        hashes,
    })
}

/// Build a canonical candidate pair.
pub(super) fn pair(left: &str, right: &str) -> CandidatePair {
    CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .expect("the fixtures use distinct identifiers")
}

/// Build an accepted pair under the given profile.
pub(super) fn accepted(
    left: &str,
    right: &str,
    profile: NormProfile,
    hashes: &[u64],
) -> AcceptedPair {
    let intersection = hashes.len();
    AcceptedPair::new(
        pair(left, right),
        profile,
        SimilarityRatio::new(intersection, intersection),
    )
}

/// Lower a span to a tree keyed by its start offset, through `trees`.
pub(super) fn lower_with(
    trees: &BTreeMap<u32, NormalizedTree>,
    span: ByteSpan,
) -> AstResult<NormalizedTree> {
    trees
        .get(&span.start())
        .cloned()
        .ok_or_else(|| AstError::UnparsableSpan {
            start: span.start(),
            end: span.end(),
        })
}

/// Refine the input with trees keyed by fragment span start.
pub(super) fn refine(
    input: &RefinementInput<'_>,
    trees: &BTreeMap<u32, NormalizedTree>,
) -> Refinement {
    match refine_with_lowering(input, &ast_config(), |_text, span| lower_with(trees, span)) {
        Ok(refined) => refined,
        Err(error) => panic!("refinement must succeed: {error}"),
    }
}

/// Build the standard two-fragment, one-accepted-pair fixture.
pub(super) fn type2_pair() -> (Vec<TokenFragment>, Vec<CandidatePair>, Vec<AcceptedPair>) {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10, &[1, 2]),
        fragment("right", NormProfile::T2, 10..20, &[1, 2]),
    ];
    (
        fragments,
        vec![pair("left", "right")],
        vec![accepted("left", "right", NormProfile::T2, &[1, 2])],
    )
}

/// Check that a rename-equivalent accepted pair is carried unchanged.
#[test]
fn equal_hashes_carry_the_token_verdict() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let [RefinedPair::Carried { ast, .. }] = refined.pairs() else {
        panic!("a rename-equivalent pair must be carried, got {refined:?}");
    };
    let Some(evidence) = ast.as_ref() else {
        panic!("conclusive evidence must be present");
    };
    assert!(evidence.hashes_equal(), "the trees hash equal");
    assert_eq!(
        evidence.cosine().get(),
        1_000_000,
        "equal hashes mean a cosine of exactly one"
    );
}

/// Check that differing trees below the threshold keep the token verdict.
#[test]
fn differing_hashes_below_the_threshold_carry() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    // Disjoint kind sets give a cosine of zero, which is below the threshold.
    trees.insert(0, synthetic_tree(1, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(100, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let [RefinedPair::Carried { ast, token }] = refined.pairs() else {
        panic!("a low-cosine pair must be carried, got {refined:?}");
    };
    assert!(
        ast.is_some(),
        "the evidence is conclusive even though the cosine is low"
    );
    assert_eq!(
        token.rule(),
        CloneRule::Type2,
        "the token verdict must stand below the threshold"
    );
}

/// Check that an all-but-one-kind histogram pair still scores above 17/20 and
/// is therefore reclassified.
#[test]
fn near_identical_structure_reclassifies() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    // One extra leaf changes the hash while the cosine stays high.
    trees.insert(10, synthetic_tree(7, NODES + 1, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let [pair] = refined.pairs() else {
        panic!("one accepted pair must refine to one outcome, got {refined:?}");
    };
    assert_eq!(
        pair.rule(),
        CloneRule::Type3,
        "a near-identical tree with a differing hash must reclassify"
    );
    assert_eq!(
        pair.rule(),
        CloneRule::Type3,
        "a reclassified result reports the stronger rule"
    );
}

/// Check that inconclusive lowering keeps the token verdict and reports no
/// AST data.
#[test]
fn inconclusive_lowering_carries_without_ast_data() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let trees = BTreeMap::new();
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let [RefinedPair::Carried { ast, token }] = refined.pairs() else {
        panic!("an inconclusive pair must be carried, got {refined:?}");
    };
    assert!(ast.is_none(), "inconclusive evidence must be absent");
    assert_eq!(token.rule(), CloneRule::Type2, "the token verdict stands");
}

/// Check that a missing parser aborts the pass.
#[test]
fn parser_unavailable_aborts() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let result = refine_with_lowering(&input, &ast_config(), |_text, _span| {
        Err(AstError::ParserUnavailable)
    });

    assert!(
        matches!(result, Err(Run1Error::ParserUnavailable)),
        "a missing parser must abort the pass, got {result:?}"
    );
}

/// Check that an invalid caller span aborts rather than becoming inconclusive.
#[test]
fn invalid_span_aborts() {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10_000, &[1]),
        fragment("other", NormProfile::T2, 10..20, &[1]),
    ];
    let candidates = vec![pair("left", "other")];
    let accepted_pairs = vec![accepted("left", "other", NormProfile::T2, &[1])];
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        Ok(synthetic_tree(7, NODES, span))
    });

    let Err(Run1Error::InvalidFragmentSpan { fragment_id, .. }) = result else {
        panic!("an out-of-bounds span must abort with a span error, got {result:?}");
    };
    assert_eq!(
        fragment_id, "left",
        "the error must name the offending fragment"
    );
}

/// Check that an inconclusive left endpoint cannot hide a right span error.
///
/// The two endpoints are validated independently. Returning as soon as the
/// left side is inconclusive would let a caller's out-of-bounds right span
/// escape validation and turn a documented abort into a carried result.
#[test]
fn right_invalid_span_aborts_past_an_inconclusive_left() {
    let fragments = vec![
        // This span is valid but lowering below refuses it, so the left side is
        // inconclusive rather than aborting.
        fragment("left", NormProfile::T2, 0..10, &[1]),
        // This span runs past the fixture source, which is the caller's error.
        fragment("other", NormProfile::T2, 10..10_000, &[1]),
    ];
    let candidates = vec![pair("left", "other")];
    let accepted_pairs = vec![accepted("left", "other", NormProfile::T2, &[1])];
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    // Refuse only the left span; anything reaching the right span must fail on
    // its own invalid range before this closure is called for it.
    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        if span.start() == 0 {
            Err(AstError::UnparsableSpan {
                start: span.start(),
                end: span.end(),
            })
        } else {
            Ok(synthetic_tree(7, NODES, span))
        }
    });

    let Err(Run1Error::InvalidFragmentSpan { fragment_id, .. }) = result else {
        panic!("the right endpoint's bad span must abort, got {result:?}");
    };
    assert_eq!(
        fragment_id, "other",
        "the error must name the right-hand fragment, not the inconclusive one"
    );
}
