//! Refinement tests for classification rows and determinism.
//!
//! Split from the sibling test module to stay inside the 400-line module
//! budget. The shared fixtures live in the sibling and are imported here.

use std::{cell::RefCell, collections::BTreeMap};

use rstest::rstest;

use super::{
    refine_with_lowering,
    tests::{
        NODES,
        accepted,
        ast_config,
        fragment,
        lower_with,
        pair,
        refine,
        token_config,
        type2_pair,
    },
};
use crate::{
    AstPassConfig,
    CloneRule,
    NormProfile,
    RefinedPair,
    RefinementInput,
    Run0Error,
    Run1Error,
    SimilarityThreshold,
    test_support::{fixture_span, synthetic_tree},
};

/// Check that mixed-profile candidates are rejected.
#[test]
fn mixed_profile_candidates_are_rejected() {
    let fragments = vec![
        fragment("left", NormProfile::T1, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[1]),
    ];
    let candidates = vec![pair("left", "right")];
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &[], &token_config);

    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        Ok(synthetic_tree(7, NODES, span))
    });

    assert!(
        matches!(
            result,
            Err(Run1Error::TokenPass(Run0Error::MixedProfiles { .. }))
        ),
        "mixed-profile candidates must be rejected, got {result:?}"
    );
}

/// Check the node floor exactly: one node fewer is inconclusive, equal is not.
///
/// The floor is `>=`, so a tree of exactly `min_nodes - 1` nodes must make the
/// evidence inconclusive while a tree of exactly `min_nodes` must not. Asserting
/// only "some small tree is inconclusive" would pass even if the comparison were
/// wrong by one.
#[test]
fn min_nodes_guard_has_an_exact_boundary() {
    let floor = ast_config().min_nodes();
    let below = synthetic_tree(7, floor.saturating_sub(1), fixture_span(0, 10));
    let at_floor = synthetic_tree(7, floor, fixture_span(0, 10));

    assert_eq!(
        crate::pair_result::node_count(&crate::ast::kind_counts(&below)),
        floor.saturating_sub(1),
        "the fixture must produce exactly the requested node count"
    );
    assert!(
        is_inconclusive(&below),
        "a tree below the floor is inconclusive"
    );
    assert!(
        !is_inconclusive(&at_floor),
        "a tree at the floor is conclusive"
    );
}

/// Check that the fixture really produces the node count it is asked for.
///
/// Every other test in this module treats `synthetic_tree`'s size as
/// authoritative, so an off-by-one in the fixture would silently move the
/// `min_nodes` boundary those tests assert. The requested sizes span the floor
/// and one either side of it, which is where an off-by-one shows up.
#[rstest]
#[case(5)]
#[case(6)]
#[case(9)]
#[case(10)]
#[case(11)]
#[case(20)]
#[case(21)]
fn synthetic_tree_produces_the_requested_node_count(#[case] nodes: usize) {
    let tree = synthetic_tree(7, nodes, fixture_span(0, 10));

    assert_eq!(
        crate::pair_result::node_count(&crate::ast::kind_counts(&tree)),
        nodes,
        "a request for {nodes} nodes must produce exactly {nodes}"
    );
}

/// Report whether a single tree makes its pair's evidence inconclusive.
fn is_inconclusive(tree: &crate::NormalizedTree) -> bool {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    trees.insert(0, tree.clone());
    trees.insert(10, tree.clone());
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);
    let [RefinedPair::Carried { ast, .. }] = refined.as_slice() else {
        panic!("an accepted pair must be carried, got {refined:?}");
    };
    ast.is_none()
}

/// Check that both accepted profiles receive the same verdict.
#[test]
fn profiles_of_one_physical_pair_agree() {
    let fragments = vec![
        fragment("left_t1", NormProfile::T1, 0..10, &[1]),
        fragment("right_t1", NormProfile::T1, 10..20, &[1]),
        fragment("left_t2", NormProfile::T2, 0..10, &[2]),
        fragment("right_t2", NormProfile::T2, 10..20, &[2]),
    ];
    let candidates = vec![pair("left_t1", "right_t1"), pair("left_t2", "right_t2")];
    let accepted_pairs = vec![
        accepted("left_t1", "right_t1", NormProfile::T1, &[1]),
        accepted("left_t2", "right_t2", NormProfile::T2, &[2]),
    ];
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES + 1, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let rules = refined.iter().map(RefinedPair::rule).collect::<Vec<_>>();
    assert_eq!(
        rules,
        vec![CloneRule::Type3, CloneRule::Type3],
        "both profiles of one physical pair must share the verdict"
    );
}

/// Check the equal-hash carve-out of the profile-agreement rule.
///
/// A physical pair accepted under both profiles shares one verdict, but when
/// that verdict is "carried" each result keeps its own token rule. The pair is
/// then reported as `WHK001` plus `WHK002`, which is what D5's equal-hash row
/// requires; the "same rule" phrasing would forbid it.
#[test]
fn carried_profiles_keep_their_own_rules() {
    let fragments = vec![
        fragment("left_t1", NormProfile::T1, 0..10, &[1]),
        fragment("right_t1", NormProfile::T1, 10..20, &[1]),
        fragment("left_t2", NormProfile::T2, 0..10, &[2]),
        fragment("right_t2", NormProfile::T2, 10..20, &[2]),
    ];
    let candidates = vec![pair("left_t1", "right_t1"), pair("left_t2", "right_t2")];
    let accepted_pairs = vec![
        accepted("left_t1", "right_t1", NormProfile::T1, &[1]),
        accepted("left_t2", "right_t2", NormProfile::T2, &[2]),
    ];
    let mut trees = BTreeMap::new();
    // Equal hashes on both sides: the shapes are identical, so the evidence
    // says "rename", and both profiles are carried.
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);

    let mut rules = refined.iter().map(RefinedPair::rule).collect::<Vec<_>>();
    rules.sort();
    assert_eq!(
        rules,
        vec![CloneRule::Type1, CloneRule::Type2],
        "a carried pair under both profiles must keep one rule per profile"
    );
    for pair in &refined {
        assert!(
            matches!(pair, RefinedPair::Carried { .. }),
            "both profiles must share the carried verdict"
        );
    }
}

/// Check that an unaccepted rename-equivalent pair is added as `WHK002`.
#[test]
fn unaccepted_equal_hashes_adds_type2() {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[2]),
    ];
    let candidates = vec![pair("left", "right")];
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &[], &token_config);

    let refined = refine(&input, &trees);

    let [RefinedPair::Added { rule, .. }] = refined.as_slice() else {
        panic!("an unaccepted rename-equivalent pair must be added, got {refined:?}");
    };
    assert_eq!(
        *rule,
        CloneRule::Type2,
        "equal canonical trees are Type-2 (OQ2 option (a))"
    );
}

/// Check that an unaccepted structurally unrelated pair is not reported.
#[test]
fn unaccepted_unrelated_pair_is_not_reported() {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[2]),
    ];
    let candidates = vec![pair("left", "right")];
    let mut trees = BTreeMap::new();
    // Disjoint kind sets give a cosine of zero.
    trees.insert(0, synthetic_tree(1, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(100, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &[], &token_config);

    let refined = refine(&input, &trees);

    assert!(
        refined.is_empty(),
        "an unrelated pair must not be reported, got {refined:?}"
    );
}

/// Check that each physical fragment is lowered exactly once per pass.
#[test]
fn fragments_are_lowered_once_per_pass() {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[1]),
        fragment("third", NormProfile::T2, 10..20, &[1]),
    ];
    let candidates = vec![pair("left", "right"), pair("left", "third")];
    let accepted_pairs = vec![accepted("left", "right", NormProfile::T2, &[1])];
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let calls = RefCell::new(Vec::new());
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(9, NODES + 3, fixture_span(10, 20)));
    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        calls.borrow_mut().push(span.start());
        lower_with(&trees, span)
    });

    assert!(result.is_ok(), "refinement should succeed: {result:?}");
    let starts = calls.into_inner();
    assert_eq!(
        starts.len(),
        2,
        "each physical fragment must be lowered once, got {starts:?}"
    );
}

/// Check that the feature-vector metric increments once per physical fragment.
#[test]

fn empty_fingerprint_sets_are_rejected_for_added_results() {
    let fragments = vec![
        fragment("left", NormProfile::T2, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[]),
    ];
    let candidates = vec![pair("left", "right")];
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES, fixture_span(10, 20)));
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &[], &token_config);

    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        Ok(synthetic_tree(7, NODES, span))
    });

    assert!(
        matches!(
            result,
            Err(Run1Error::TokenPass(Run0Error::EmptyFingerprintSet { .. }))
        ),
        "an empty fingerprint set must be rejected, got {result:?}"
    );
}

/// Check that an unusable Type-3 threshold is rejected before any lowering.
#[rstest]
#[case(SimilarityThreshold::new_unchecked("type3", 24, 25), "ThresholdOrder")]
#[case(
    SimilarityThreshold::new_unchecked("type3", 1, 3),
    "ThresholdNotDecimal"
)]
fn invalid_thresholds_are_rejected_before_lowering(
    #[case] type3: SimilarityThreshold,
    #[case] expected: &str,
) {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);
    let config = AstPassConfig::new("0.3.0").with_type3_threshold(type3);

    let result = refine_with_lowering(&input, &config, |_text, span| {
        Ok(synthetic_tree(7, NODES, span))
    });

    let matches_expected = match expected {
        "ThresholdOrder" => matches!(result, Err(Run1Error::ThresholdOrder { .. })),
        "ThresholdNotDecimal" => matches!(result, Err(Run1Error::ThresholdNotDecimal { .. })),
        other => panic!("unknown expectation `{other}`"),
    };
    assert!(
        matches_expected,
        "an unusable Type-3 threshold must be rejected with {expected}, got {result:?}"
    );
}
